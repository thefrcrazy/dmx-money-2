import { DurableObject } from 'cloudflare:workers';
import { timingSafeEqual } from 'node:crypto';

const MAX_FRAME = 12 * 1024 * 1024;
const MAX_PENDING = 8;
const WAIT_MS = 25_000;
const ID = /^[a-f0-9]{32}$/;
const HASH = /^[A-Za-z0-9_-]{43}$/;
const REQUEST_ID = /^[0-9a-f-]{36}$/;

type Enrollment = { desktopHash: string; mobileHash: string };
type Envelope = { id: string; nonce: string; ciphertext: string };
type Pending = { resolve: (response: Response) => void; timer: ReturnType<typeof setTimeout> };

function json(value: unknown, status = 200): Response {
  return Response.json(value, { status, headers: { 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' } });
}

async function limitedText(request: Request, limit: number): Promise<string | null> {
  if (!request.body) return '';
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > limit) { await reader.cancel(); return null; }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
}

function envelope(text: string): Envelope | null {
  try {
    const value: unknown = JSON.parse(text);
    if (!value || typeof value !== 'object') return null;
    const candidate = value as Partial<Envelope>;
    return typeof candidate.id === 'string' && REQUEST_ID.test(candidate.id)
      && typeof candidate.nonce === 'string' && /^[A-Za-z0-9_-]{16}$/.test(candidate.nonce)
      && typeof candidate.ciphertext === 'string' && /^[A-Za-z0-9_-]{22,}$/.test(candidate.ciphertext)
      ? { id: candidate.id, nonce: candidate.nonce, ciphertext: candidate.ciphertext } : null;
  } catch { return null; }
}

async function tokenHash(request: Request): Promise<string> {
  const auth = request.headers.get('authorization') ?? '';
  if (!/^Bearer [A-Za-z0-9_-]{43}$/.test(auth)) return '';
  const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(auth.slice(7)));
  return btoa(String.fromCharCode(...new Uint8Array(bytes))).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}

function equal(left: string, right: string): boolean {
  const encoder = new TextEncoder();
  const a = encoder.encode(left), b = encoder.encode(right);
  return a.length === b.length && timingSafeEqual(a, b);
}

/** One desktop per object. Only credential hashes persist; financial payloads never do. */
export class DeviceRelay extends DurableObject<Env> {
  private enrollment?: Enrollment;
  private readonly pending = new Map<string, Pending>();

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS rate_window (minute INTEGER PRIMARY KEY, count INTEGER NOT NULL)');
    ctx.blockConcurrencyWhile(async () => {
      this.enrollment = await ctx.storage.get<Enrollment>('enrollment');
    });
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'));
  }

  async fetch(request: Request): Promise<Response> {
    try {
      const path = new URL(request.url).pathname;
      if (path.endsWith('/enroll') && request.method === 'POST') {
        const text = await limitedText(request, 1024);
        if (text === null) return json({ error: 'too_large' }, 413);
        const body = JSON.parse(text) as Partial<Enrollment>;
        if (!body || !HASH.test(body.desktopHash ?? '') || !HASH.test(body.mobileHash ?? '')) return json({ error: 'invalid_enrollment' }, 400);
        if (this.enrollment) {
          return equal(await tokenHash(request), this.enrollment.desktopHash)
            && equal(body.desktopHash!, this.enrollment.desktopHash) && equal(body.mobileHash!, this.enrollment.mobileHash)
            ? json({ ok: true }) : json({ error: 'already_enrolled' }, 409);
        }
        // Validate possession and atomically claim the random device ID. An enrollment cannot
        // replace another device even if concurrent requests arrive during the storage await.
        if (!equal(await tokenHash(request), body.desktopHash!)) return json({ error: 'unauthorized' }, 401);
        const record = { desktopHash: body.desktopHash!, mobileHash: body.mobileHash! };
        const claimed = await this.ctx.storage.transaction(async storage => {
          if (await storage.get('enrollment')) return false;
          await storage.put('enrollment', record);
          return true;
        });
        if (!claimed) return json({ error: 'already_enrolled' }, 409);
        this.enrollment = record;
        return json({ ok: true }, 201);
      }
      if (!this.enrollment) return json({ error: 'not_found' }, 404);
      const hash = await tokenHash(request);
      if (path.endsWith('/enroll') && request.method === 'DELETE') {
        if (!equal(hash, this.enrollment.desktopHash)) return json({ error: 'unauthorized' }, 401);
        await this.ctx.blockConcurrencyWhile(async () => {
          await this.ctx.storage.deleteAll();
          // deleteAll also drops SQLite tables. This object may be enrolled again
          // without a new constructor, so restore its empty rate-limit schema.
          this.ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS rate_window (minute INTEGER PRIMARY KEY, count INTEGER NOT NULL)');
          this.enrollment = undefined;
          for (const id of this.pending.keys()) this.finish(id, json({ error: 'desktop_offline' }, 503));
          for (const socket of this.ctx.getWebSockets('desktop')) socket.close(1000, 'unenrolled');
        });
        return new Response(null, { status: 204, headers: { 'cache-control': 'no-store' } });
      }
      if (path.endsWith('/connect') && request.method === 'GET') {
        if (!equal(hash, this.enrollment.desktopHash)) return json({ error: 'unauthorized' }, 401);
        if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return json({ error: 'websocket_required' }, 426);
        for (const previous of this.ctx.getWebSockets('desktop')) previous.close(1000, 'reconnected');
        const [client, server] = Object.values(new WebSocketPair());
        this.ctx.acceptWebSocket(server, ['desktop']);
        return new Response(null, { status: 101, webSocket: client });
      }
      if (!path.endsWith('/request') || request.method !== 'POST') return json({ error: 'not_found' }, 404);
      if (!equal(hash, this.enrollment.mobileHash)) return json({ error: 'unauthorized' }, 401);
      const minute = Math.floor(Date.now() / 60_000);
      this.ctx.storage.sql.exec('DELETE FROM rate_window WHERE minute < ?', minute);
      this.ctx.storage.sql.exec('INSERT INTO rate_window (minute, count) VALUES (?, 1) ON CONFLICT(minute) DO UPDATE SET count=count+1', minute);
      const rate = this.ctx.storage.sql.exec<{count: number}>('SELECT count FROM rate_window WHERE minute=?', minute).one();
      if (rate.count > 300 || this.pending.size >= MAX_PENDING) return json({ error: 'rate_limited' }, 429);
      const desktop = this.ctx.getWebSockets('desktop')[0];
      if (!desktop || desktop.readyState !== WebSocket.OPEN) return json({ error: 'desktop_offline' }, 503);
      const text = await limitedText(request, MAX_FRAME);
      if (text === null) return json({ error: 'too_large' }, 413);
      const packet = envelope(text);
      if (!packet) return json({ error: 'invalid_packet' }, 400);
      // Recheck after reading the streamed body: multiple requests may have arrived meanwhile.
      if (this.pending.size >= MAX_PENDING) return json({ error: 'rate_limited' }, 429);
      if (this.pending.has(packet.id)) return json({ error: 'duplicate_request' }, 409);
      return await new Promise<Response>(resolve => {
        const timer = setTimeout(() => {
          this.pending.delete(packet.id);
          resolve(json({ error: 'desktop_timeout' }, 504));
        }, WAIT_MS);
        this.pending.set(packet.id, { resolve, timer });
        try { desktop.send(JSON.stringify(packet)); }
        catch { this.finish(packet.id, json({ error: 'desktop_offline' }, 503)); }
      });
    } catch { return json({ error: 'invalid_request' }, 400); }
  }

  webSocketMessage(socket: WebSocket, message: string | ArrayBuffer): void {
    if (!this.ctx.getWebSockets('desktop').includes(socket)) return;
    if (typeof message !== 'string' || new TextEncoder().encode(message).length > MAX_FRAME) {
      socket.close(1009, 'frame too large'); return;
    }
    const packet = envelope(message);
    if (packet) this.finish(packet.id, json(packet));
  }

  webSocketClose(socket: WebSocket): void {
    if (this.ctx.getWebSockets('desktop').some(current => current !== socket && current.readyState === WebSocket.OPEN)) return;
    for (const id of this.pending.keys()) this.finish(id, json({ error: 'desktop_offline' }, 503));
  }

  webSocketError(socket: WebSocket): void { this.webSocketClose(socket); }

  private finish(id: string, response: Response): void {
    const pending = this.pending.get(id);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.pending.delete(id);
    pending.resolve(response);
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === '/relay/capabilities') return json({ protocol: 'dmx-relay-v1' });
    const route = /^\/relay\/([a-f0-9]{32})\/(enroll|connect|request)$/.exec(url.pathname);
    if (route) {
      if (!ID.test(route[1])) return json({ error: 'not_found' }, 404);
      // Browser requests must come from the single companion origin; the desktop uses no Origin.
      const origin = request.headers.get('origin');
      if (origin && origin !== url.origin) return json({ error: 'forbidden_origin' }, 403);
      if (route[2] === 'enroll') {
        const result = await env.ENROLLMENT_LIMIT.limit({ key: request.headers.get('cf-connecting-ip') ?? 'unknown' });
        if (!result.success) return json({ error: 'rate_limited' }, 429);
      }
      return env.RELAYS.getByName(route[1]).fetch(request);
    }
    if (url.pathname.startsWith('/relay/')) return json({ error: 'not_found' }, 404);
    if (url.pathname === '/' || url.pathname === '/mobile') return Response.redirect(`${url.origin}/mobile/`, 302);
    if (!url.pathname.startsWith('/mobile/')) return json({ error: 'not_found' }, 404);
    const assetUrl = new URL(request.url);
    assetUrl.pathname = assetUrl.pathname.slice('/mobile'.length);
    const response = await env.ASSETS.fetch(new Request(assetUrl, request));
    const headers = new Headers(response.headers);
    headers.set('x-content-type-options', 'nosniff');
    headers.set('referrer-policy', 'no-referrer');
    headers.set('content-security-policy', "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'none'");
    if (url.pathname.endsWith('/sw.js') || url.pathname.endsWith('.html') || url.pathname === '/mobile' || url.pathname === '/mobile/') headers.set('cache-control', 'no-store');
    return new Response(response.body, { status: response.status, headers });
  },
} satisfies ExportedHandler<Env>;
