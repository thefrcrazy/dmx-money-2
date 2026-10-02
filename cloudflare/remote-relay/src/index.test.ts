import { SELF } from 'cloudflare:test';
import { expect, test } from 'vitest';

const base = 'https://relay.test';
const token = 'a'.repeat(43);
const mobile = 'b'.repeat(43);
const hash = async (value: string) => {
  const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value));
  return btoa(String.fromCharCode(...new Uint8Array(bytes))).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
};
const endpoint = () => `${base}/relay/${crypto.randomUUID().replaceAll('-', '')}`;
const enrollment = async (url: string, auth = token) => SELF.fetch(`${url}/enroll`, {
  method: 'POST', headers: { authorization: `Bearer ${auth}`, 'cf-connecting-ip': crypto.randomUUID() },
  body: JSON.stringify({ desktopHash: await hash(token), mobileHash: await hash(mobile) }),
});

test('discovers protocol and rejects cross-origin enrollment', async () => {
  expect(await (await SELF.fetch(`${base}/relay/capabilities`)).json()).toEqual({ protocol: 'dmx-relay-v1' });
  const response = await SELF.fetch(`${endpoint()}/enroll`, { method: 'POST', headers: { origin: 'https://attacker.test' }, body: '{}' });
  expect(response.status).toBe(403);
});

test('Pages CORS accepts only encrypted mobile POSTs and rejects foreign origins', async () => {
  const url = endpoint();
  const origin = 'https://dmxmoney-companion.pages.dev';
  const preflight = (requestOrigin: string, headers = 'authorization,content-type', method = 'POST') => SELF.fetch(`${url}/request`, {
    method: 'OPTIONS', headers: { origin: requestOrigin, 'access-control-request-method': method, 'access-control-request-headers': headers },
  });
  const allowed = await preflight(origin);
  expect(allowed.status).toBe(204);
  expect(allowed.headers.get('access-control-allow-origin')).toBe(origin);
  expect(allowed.headers.get('access-control-allow-credentials')).toBeNull();
  expect(allowed.headers.get('vary')).toBe('Origin');
  expect((await preflight('https://attacker.test')).status).toBe(403);
  expect((await preflight(origin, 'x-unexpected')).status).toBe(403);
  expect((await preflight(origin, 'authorization', 'DELETE')).status).toBe(403);
  expect((await SELF.fetch(`${url}/enroll`, { method: 'POST', headers: { origin }, body: '{}' })).status).toBe(403);
  expect((await enrollment(url)).status).toBe(201);
  const unauthorized = await SELF.fetch(`${url}/request`, { method: 'POST', headers: { origin, authorization: `Bearer ${token}` }, body: '{}' });
  expect(unauthorized.status).toBe(401);
  expect(unauthorized.headers.get('access-control-allow-origin')).toBe(origin);
  const unavailable = await SELF.fetch(`${url}/request`, { method: 'POST', headers: { origin, authorization: `Bearer ${mobile}` }, body: '{}' });
  expect(unavailable.status).toBe(503);
  expect(unavailable.headers.get('access-control-allow-origin')).toBe(origin);
});

test('enrollment proves possession and cannot replace credentials', async () => {
  const url = endpoint();
  expect((await enrollment(url, mobile)).status).toBe(401);
  expect((await enrollment(url)).status).toBe(201);
  expect((await enrollment(url)).status).toBe(200);
  expect((await enrollment(url, mobile)).status).toBe(409);
  const replaced = await SELF.fetch(`${url}/enroll`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'cf-connecting-ip': crypto.randomUUID() }, body: JSON.stringify({ desktopHash: await hash(token), mobileHash: await hash(token) }) });
  expect(replaced.status).toBe(409);
});

test('authorization isolates desktop and mobile, offline desktop is explicit', async () => {
  const url = endpoint();
  expect((await enrollment(url)).status).toBe(201);
  expect((await SELF.fetch(`${url}/connect`, { headers: { authorization: `Bearer ${mobile}`, upgrade: 'websocket' } })).status).toBe(401);
  expect((await SELF.fetch(`${url}/request`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'cf-connecting-ip': crypto.randomUUID() }, body: '{}' })).status).toBe(401);
  expect((await SELF.fetch(`${url}/request`, { method: 'POST', headers: { authorization: `Bearer ${mobile}` }, body: '{}' })).status).toBe(503);
});

test('routes opaque encrypted packets and replies through outbound desktop websocket', async () => {
  const url = endpoint();
  // Unique IP prevents unrelated tests from sharing the rate-limit window.
  const enrollResponse = await SELF.fetch(`${url}/enroll`, {
    method: 'POST', headers: { authorization: `Bearer ${token}`, 'cf-connecting-ip': '192.0.2.42' },
    body: JSON.stringify({ desktopHash: await hash(token), mobileHash: await hash(mobile) }),
  });
  expect(enrollResponse.status).toBe(201);
  const response = await SELF.fetch(`${url}/connect`, { headers: { authorization: `Bearer ${token}`, upgrade: 'websocket' } });
  expect(response.status).toBe(101);
  const socket = response.webSocket!;
  socket.accept();
  const packet = { id: crypto.randomUUID(), nonce: 'A'.repeat(16), ciphertext: 'B'.repeat(32) };
  const received = new Promise<MessageEvent>(resolve => socket.addEventListener('message', resolve, { once: true }));
  const reply = SELF.fetch(`${url}/request`, { method: 'POST', headers: { authorization: `Bearer ${mobile}` }, body: JSON.stringify(packet) });
  expect(JSON.parse((await received).data)).toEqual(packet);
  const answer = { ...packet, ciphertext: 'C'.repeat(32) };
  socket.send(JSON.stringify(answer));
  expect(await (await reply).json()).toEqual(answer);
  socket.close();
});

test('streams enrollment with a hard size limit without trusting Content-Length', async () => {
  const response = await SELF.fetch(`${endpoint()}/enroll`, {
    method: 'POST', headers: { 'cf-connecting-ip': '192.0.2.43' }, body: 'x'.repeat(2048),
  });
  expect(response.status).toBe(413);
});

test('only the desktop owner can remove the relay credentials', async () => {
  const url = endpoint();
  expect((await enrollment(url)).status).toBe(201);
  expect((await SELF.fetch(`${url}/enroll`, { method: 'DELETE', headers: { authorization: `Bearer ${mobile}` } })).status).toBe(401);
  expect((await SELF.fetch(`${url}/enroll`, { method: 'DELETE', headers: { authorization: `Bearer ${token}` } })).status).toBe(204);
  expect((await SELF.fetch(`${url}/request`, { method: 'POST', headers: { authorization: `Bearer ${mobile}` }, body: '{}' })).status).toBe(404);
  expect((await enrollment(url)).status).toBe(201);
  expect((await SELF.fetch(`${url}/request`, { method: 'POST', headers: { authorization: `Bearer ${mobile}` }, body: '{}' })).status).toBe(503);
});
