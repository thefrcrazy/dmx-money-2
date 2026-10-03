import { DurableObject } from 'cloudflare:workers';
import { timingSafeEqual } from 'node:crypto';
import { BodyReadTimeout, limitedText, RequestBudget, reservedResponse } from './requestBudget';

const MAX_FRAME = 12 * 1024 * 1024;
// Each tiny request can produce a full 12-Mio reply. Two fixed reservations leave
// conservative headroom for decoding/validation and runtime buffers; not a heap-size proof.
const MAX_PENDING = 2;
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

function busy(): Response {
  const response = json({ error: 'relay_busy' }, 503);
  response.headers.set('retry-after', '1');
  return response;
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
  private readonly budget = new RequestBudget(MAX_PENDING, 2 * MAX_FRAME);

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
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
      // Unknown device IDs never write a schema; only an authenticated mobile does.
      this.ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS rate_window (minute INTEGER PRIMARY KEY, count INTEGER NOT NULL)');
      const minute = Math.floor(Date.now() / 60_000);
      this.ctx.storage.sql.exec('DELETE FROM rate_window WHERE minute < ?', minute);
      this.ctx.storage.sql.exec('INSERT INTO rate_window (minute, count) VALUES (?, 1) ON CONFLICT(minute) DO UPDATE SET count=count+1', minute);
      const rate = this.ctx.storage.sql.exec<{count: number}>('SELECT count FROM rate_window WHERE minute=?', minute).one();
      if (rate.count > 300) return json({ error: 'rate_limited' }, 429);
      if (this.pending.size >= MAX_PENDING) return busy();
      const desktop = this.ctx.getWebSockets('desktop')[0];
      if (!desktop || desktop.readyState !== WebSocket.OPEN) return json({ error: 'desktop_offline' }, 503);
      const reservation = this.budget.reserve(MAX_FRAME);
      if (!reservation) return busy();
      let transferred = false;
      try {
        const text = await limitedText(request, MAX_FRAME);
        if (text === null) return json({ error: 'too_large' }, 413);
        const packet = envelope(text);
        if (!packet) return json({ error: 'invalid_packet' }, 400);
        // Recheck after reading the streamed body: requests may have arrived meanwhile.
        if (this.pending.size >= MAX_PENDING) return busy();
        if (this.pending.has(packet.id)) return json({ error: 'duplicate_request' }, 409);
        const response = await new Promise<Response>(resolve => {
          const timer = setTimeout(() => this.finish(packet.id, json({ error: 'desktop_timeout' }, 504)), WAIT_MS);
          this.pending.set(packet.id, { resolve, timer });
          try { desktop.send(JSON.stringify(packet)); }
          catch { this.finish(packet.id, json({ error: 'desktop_offline' }, 503)); }
        });
        const output = reservedResponse(response, reservation);
        transferred = true;
        return output;
      } finally { if (!transferred) reservation.release(); }
    } catch (error) { return error instanceof BodyReadTimeout
      ? json({ error: 'body_timeout' }, 408)
      : json({ error: 'invalid_request' }, 400); }
  }

  webSocketMessage(socket: WebSocket, message: string | ArrayBuffer): void {
    if (!this.ctx.getWebSockets('desktop').includes(socket)) return;
    if (typeof message !== 'string' || new TextEncoder().encode(message).length > MAX_FRAME) {
      socket.close(1009, 'frame too large'); return;
    }
    const packet = envelope(message);
    if (packet && this.pending.has(packet.id)) {
      // Forward the validated encrypted text directly, without another large JSON stringify.
      this.finish(packet.id, new Response(message, { headers: {
        'content-type': 'application/json', 'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
      } }));
    }
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
      // The desktop has no Origin. Pages is the one additional browser origin;
      // legacy Worker-hosted companions remain valid until explicit re-pairing.
      const origin = request.headers.get('origin');
      const pagesOrigin = origin === env.COMPANION_ORIGIN;
      if (origin && origin !== url.origin && !(pagesOrigin && route[2] === 'request')) return json({ error: 'forbidden_origin' }, 403);
      if (request.method === 'OPTIONS') {
        const method = request.headers.get('access-control-request-method');
        const requestedHeaders = (request.headers.get('access-control-request-headers') ?? '')
          .split(',').map(value => value.trim().toLowerCase()).filter(Boolean);
        if (!pagesOrigin || route[2] !== 'request' || method !== 'POST'
          || requestedHeaders.some(header => !['authorization', 'content-type'].includes(header))) {
          return json({ error: 'forbidden_preflight' }, 403);
        }
        return new Response(null, { status: 204, headers: {
          'access-control-allow-origin': env.COMPANION_ORIGIN,
          'access-control-allow-methods': 'POST',
          'access-control-allow-headers': 'Authorization, Content-Type',
          'access-control-max-age': '600', 'vary': 'Origin', 'cache-control': 'no-store',
        } });
      }
      const browserResponse = (response: Response) => {
        if (!pagesOrigin) return response;
        const headers = new Headers(response.headers);
        headers.set('access-control-allow-origin', env.COMPANION_ORIGIN);
        headers.set('vary', 'Origin');
        return new Response(response.body, { status: response.status, headers });
      };
      const expectedMethod = route[2] === 'connect' ? request.method === 'GET'
        : route[2] === 'request' ? request.method === 'POST'
        : request.method === 'POST' || request.method === 'DELETE';
      if (!expectedMethod) return browserResponse(json({ error: 'method_not_allowed' }, 405));
      // Limit all device IDs before constructing a Durable Object. Rate bindings
      // share counters within each Cloudflare location, not a global exact quota.
      const ip = request.headers.get('cf-connecting-ip') ?? 'unknown';
      const [clientLimit, serviceLimit] = await Promise.all([
        env.REQUEST_LIMIT.limit({ key: ip }), env.SERVICE_LIMIT.limit({ key: 'all-relays' }),
      ]);
      if (!clientLimit.success || !serviceLimit.success) return browserResponse(json({ error: 'rate_limited' }, 429));
      if (route[2] !== 'enroll' && !/^Bearer [A-Za-z0-9_-]{43}$/.test(request.headers.get('authorization') ?? '')) {
        return browserResponse(json({ error: 'unauthorized' }, 401));
      }
      if (route[2] === 'enroll') {
        const result = await env.ENROLLMENT_LIMIT.limit({ key: request.headers.get('cf-connecting-ip') ?? 'unknown' });
        if (!result.success) return json({ error: 'rate_limited' }, 429);
      }
      return browserResponse(await env.RELAYS.getByName(route[1]).fetch(request));
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
