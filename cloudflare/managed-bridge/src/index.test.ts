import { expect, test } from 'bun:test';
import worker from './index';
type Env = Parameters<typeof worker.fetch>[1];
const request = (body: string, secret = '') => new Request('https://test.invalid/v1/devices/register', {
  method: 'POST', body, headers: { authorization: `Bearer ${secret}` },
});
test('rejects oversized bodies without trusting Content-Length', async () => {
  const response = await worker.fetch(request(JSON.stringify({ localIp: 'x'.repeat(32768) })), {} as Env);
  expect(response.status).toBe(413);
});
test('rejects malformed JSON and wrong field types', async () => {
  for (const body of ['{', 'null', '[]', '{"existingDeviceId":42}']) {
    expect((await worker.fetch(request(body), {} as Env)).status).toBe(400);
  }
});
test('shared registration secret cannot replace an existing device', async () => {
  const hash = Buffer.from(await crypto.subtle.digest('SHA-256', new TextEncoder().encode('device-secret'))).toString('base64url');
  let writes = 0;
  const env = {
    REGISTRATION_SECRET: 'shared-secret',
    DEVICES: { get: async () => ({ id: 'device123', secretHash: hash }), put: async () => { writes++; } },
  } as unknown as Env;
  const response = await worker.fetch(request('{"existingDeviceId":"device123"}', 'shared-secret'), env);
  expect(response.status).toBe(401);
  expect(writes).toBe(0);
});

test('accepts null optional IDs used by the Rust client', async () => {
  expect((await worker.fetch(request('{"existingDeviceId":null,"localIp":"192.168.1.2"}'), {} as Env)).status).toBe(401);
});
test('device owner can still rotate its own credentials', async () => {
  const hash = Buffer.from(await crypto.subtle.digest('SHA-256', new TextEncoder().encode('device-secret'))).toString('base64url');
  const records = new Map<string, unknown>([['device:device123', { id: 'device123', secretHash: hash, createdAt: '2026-09-16', txtRecordIds: [] }]]);
  const env = {
    BRIDGE_DOMAIN: 'example.com', PWA_URL: 'https://example.com/mobile',
    DEVICES: {
      get: async (key: string) => records.get(key) ?? null,
      put: async (key: string, value: string) => { records.set(key, JSON.parse(value)); },
    },
  } as unknown as Env;
  const response = await worker.fetch(request('{"existingDeviceId":"device123","localIp":"192.168.1.2"}', 'device-secret'), env);
  expect(response.status).toBe(200);
  const result = await response.json() as { deviceId: string; deviceSecret: string };
  expect(result.deviceId).toBe('device123');
  expect(result.deviceSecret).not.toBe('device-secret');
});
