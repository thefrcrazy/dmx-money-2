type KVNamespace = {
  get<T>(key: string, type: "json"): Promise<T | null>;
  get(key: string, type: "text"): Promise<string | null>;
  get(key: string, type: "arrayBuffer"): Promise<ArrayBuffer | null>;
  put(key: string, value: string, options?: { expirationTtl?: number }): Promise<void>;
};

type Env = {
  DEVICES: KVNamespace;
  ASSETS: KVNamespace;
  CLOUDFLARE_API_TOKEN: string;
  CLOUDFLARE_ZONE_ID: string;
  BRIDGE_DOMAIN: string;
  PWA_URL: string;
  REGISTRATION_SECRET?: string;
  DEVICE_PREFIX?: string;
  REGISTRATION_LIMIT_PER_HOUR?: string;
};

type DeviceRecord = {
  id: string;
  secretHash: string;
  domain: string;
  appUrl: string;
  localHost: string;
  aRecordId?: string;
  txtRecordIds: string[];
  createdAt: string;
  updatedAt: string;
  lastLocalIp?: string;
};

type RegisterBody = {
  existingDeviceId?: string;
  localIp?: string;
};

type CloudflareResponse<T> = {
  success: boolean;
  result?: T;
  errors?: Array<{ message: string }>;
};

type CloudflareDnsRecord = {
  id: string;
};

const JSON_HEADERS = {
  "content-type": "application/json; charset=utf-8",
  "cache-control": "no-store",
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      const url = new URL(request.url);
      if (request.method === "OPTIONS") {
        return new Response(null, { status: 204, headers: JSON_HEADERS });
      }
      if (request.method === "GET" || request.method === "HEAD") {
        return serveStaticAsset(request, env);
      }
      if (request.method !== "POST") {
        return jsonResponse({ error: "method_not_allowed" }, 405);
      }

      const route = matchRoute(url.pathname);
      if (!route) {
        return jsonResponse({ error: "not_found" }, 404);
      }

      if (route.name === "register") {
        const body = await readJson<RegisterBody>(request);
        const registrationAuth = await authorizeRegistration(request, env, body);
        if (registrationAuth) {
          return registrationAuth;
        }
        const rateLimit = await registrationRateLimit(request, env);
        if (rateLimit) {
          return rateLimit;
        }
        return await registerDevice(body, env);
      }

      const device = await authorizeDevice(request, env, route.deviceId);
      if (device.ok === false) {
        return device.response;
      }

      if (route.name === "dns") {
        return await updateDeviceDns(request, env, device.record);
      }
      if (route.name === "txt") {
        return await presentAcmeTxt(request, env, device.record);
      }
      return await deleteAcmeTxt(request, env, device.record);
    } catch (error) {
      console.error("managed_bridge_error", safeError(error));
      return jsonResponse({ error: "internal_error" }, 500);
    }
  },
};

async function serveStaticAsset(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname === "/") {
    return Response.redirect(`${url.origin}/mobile`, 302);
  }

  const key = staticAssetKey(url.pathname);
  const body = await env.ASSETS.get(key, "arrayBuffer");
  if (body === null) {
    const fallback = await env.ASSETS.get("index.html", "text");
    if (fallback !== null && acceptsHtml(request)) {
      return new Response(request.method === "HEAD" ? null : fallback, {
        headers: staticHeaders("index.html", env),
      });
    }
    return new Response("Not found", { status: 404 });
  }

  return new Response(request.method === "HEAD" ? null : body, {
    headers: staticHeaders(key, env),
  });
}

function staticAssetKey(pathname: string): string {
  const cleaned = pathname.replace(/^\/+/, "");
  if (cleaned === "mobile" || cleaned === "mobile/") {
    return "index.html";
  }
  if (cleaned.startsWith("mobile/assets/")) {
    return cleaned.replace(/^mobile\//, "");
  }
  if (cleaned.startsWith("mobile/") && !cleaned.includes(".")) {
    return "index.html";
  }
  return cleaned || "index.html";
}

function staticHeaders(key: string, env: Env): Headers {
  const headers = new Headers();
  const bridgeDomain = safeCspDomain(env.BRIDGE_DOMAIN);
  headers.set("content-type", contentType(key));
  headers.set("x-content-type-options", "nosniff");
  headers.set("referrer-policy", "strict-origin-when-cross-origin");
  headers.set(
    "content-security-policy",
    [
      "default-src 'self'",
      "base-uri 'self'",
      "object-src 'none'",
      "frame-ancestors 'none'",
      "script-src 'self' 'unsafe-inline'",
      "style-src 'self' 'unsafe-inline'",
      "img-src 'self' data: blob:",
      "font-src 'self' data:",
      "media-src 'self' blob:",
      `connect-src 'self' https://*.sync.${bridgeDomain}:*`,
      "worker-src 'self'",
      "manifest-src 'self'",
      "form-action 'none'",
      "upgrade-insecure-requests",
    ].join("; "),
  );
  headers.set("cache-control", key.includes("-") && key.startsWith("assets/")
    ? "public, max-age=31536000, immutable"
    : "no-cache");
  return headers;
}

function safeCspDomain(value: string): string {
  const domain = value.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/\/.*$/, "");
  return /^[a-z0-9.-]+$/.test(domain) && domain.includes(".") ? domain : "develop-max.com";
}

function contentType(key: string): string {
  if (key.endsWith(".html")) return "text/html; charset=utf-8";
  if (key.endsWith(".js")) return "application/javascript; charset=utf-8";
  if (key.endsWith(".css")) return "text/css; charset=utf-8";
  if (key.endsWith(".json") || key.endsWith(".webmanifest")) return "application/manifest+json; charset=utf-8";
  if (key.endsWith(".png")) return "image/png";
  if (key.endsWith(".svg")) return "image/svg+xml";
  if (key.endsWith(".ico")) return "image/x-icon";
  return "application/octet-stream";
}

function acceptsHtml(request: Request): boolean {
  return request.headers.get("accept")?.includes("text/html") ?? false;
}

function matchRoute(pathname: string):
  | { name: "register" }
  | { name: "dns" | "txt" | "deleteTxt"; deviceId: string }
  | null {
  if (pathname === "/v1/devices/register") {
    return { name: "register" };
  }

  const parts = pathname.split("/").filter(Boolean);
  if (parts.length < 4 || parts[0] !== "v1" || parts[1] !== "devices") {
    return null;
  }

  const deviceId = parts[2];
  if (parts.length === 4 && parts[3] === "dns") {
    return { name: "dns", deviceId };
  }
  if (parts.length === 5 && parts[3] === "acme" && parts[4] === "txt") {
    return { name: "txt", deviceId };
  }
  if (parts.length === 6 && parts[3] === "acme" && parts[4] === "txt" && parts[5] === "delete") {
    return { name: "deleteTxt", deviceId };
  }
  return null;
}

async function authorizeRegistration(
  request: Request,
  env: Env,
  body: RegisterBody,
): Promise<Response | null> {
  const header = request.headers.get("authorization") || "";
  const provided = header.startsWith("Bearer ") ? header.slice(7).trim() : "";
  if (!provided) {
    return jsonResponse({ error: "unauthorized" }, 401);
  }
  const providedHash = await hashSecret(provided);

  // A device that already registered renews its own credentials with the secret it
  // holds. Without this an installation that lost the shared registration secret
  // could never rotate, and every pairing attempt answered 401.
  const existing = normalizeId(body.existingDeviceId);
  if (existing) {
    const record = await env.DEVICES.get<DeviceRecord>(deviceKey(existing), "json");
    if (record && timingSafeEqual(providedHash, record.secretHash)) {
      return null;
    }
  }

  const secret = env.REGISTRATION_SECRET?.trim();
  if (!secret) {
    return jsonResponse({ error: "registration_disabled" }, 403);
  }
  if (!timingSafeEqual(providedHash, await hashSecret(secret))) {
    return jsonResponse({ error: "unauthorized" }, 401);
  }

  return null;
}

async function registerDevice(body: RegisterBody, env: Env): Promise<Response> {
  const existing = normalizeId(body.existingDeviceId);
  const reusable = existing ? await env.DEVICES.get<DeviceRecord>(deviceKey(existing), "json") : null;
  const id = existing || crypto.randomUUID().replaceAll("-", "").slice(0, 20);
  const secret = randomBase64Url(32);
  const domain = normalizeDomain(env.BRIDGE_DOMAIN);
  const appUrl = normalizeAppUrl(env.PWA_URL);
  const prefix = normalizeLabel(env.DEVICE_PREFIX || "dmx");
  const localHost = `${prefix}-${id}.sync.${domain}`;
  const now = new Date().toISOString();
  const record: DeviceRecord = {
    id,
    secretHash: await hashSecret(secret),
    domain,
    appUrl,
    localHost,
    aRecordId: reusable?.aRecordId,
    txtRecordIds: reusable?.txtRecordIds || [],
    createdAt: reusable?.createdAt || now,
    updatedAt: now,
    lastLocalIp: isPrivateIp(body.localIp || "") ? body.localIp : undefined,
  };

  await env.DEVICES.put(deviceKey(id), JSON.stringify(record));
  return jsonResponse({
    deviceId: id,
    deviceSecret: secret,
    domain,
    appUrl,
    localHost,
  });
}

async function registrationRateLimit(request: Request, env: Env): Promise<Response | null> {
  const limit = Number(env.REGISTRATION_LIMIT_PER_HOUR || "20");
  const ip = request.headers.get("cf-connecting-ip") || "unknown";
  const hour = Math.floor(Date.now() / 3_600_000);
  const key = `rate:register:${ip}:${hour}`;
  const current = (await env.DEVICES.get<{ count: number }>(key, "json"))?.count || 0;
  if (current >= limit) {
    return jsonResponse({ error: "rate_limited" }, 429);
  }
  await env.DEVICES.put(key, JSON.stringify({ count: current + 1 }), { expirationTtl: 3900 });
  return null;
}

async function updateDeviceDns(
  request: Request,
  env: Env,
  device: DeviceRecord,
): Promise<Response> {
  const body = await readJson<{ localIp?: string; localHost?: string }>(request);
  if (!body.localIp || !isPrivateIp(body.localIp)) {
    return jsonResponse({ error: "invalid_private_ip" }, 400);
  }
  if (body.localHost !== device.localHost) {
    return jsonResponse({ error: "invalid_local_host" }, 400);
  }

  const recordId = await upsertDnsRecord(env, {
    id: device.aRecordId,
    type: "A",
    name: device.localHost,
    content: body.localIp,
  });
  await saveDevice(env, {
    ...device,
    aRecordId: recordId,
    lastLocalIp: body.localIp,
    updatedAt: new Date().toISOString(),
  });
  return jsonResponse({ recordId });
}

async function presentAcmeTxt(
  request: Request,
  env: Env,
  device: DeviceRecord,
): Promise<Response> {
  const body = await readJson<{ name?: string; value?: string }>(request);
  const expectedName = `_acme-challenge.${device.localHost}`;
  if (body.name !== expectedName || !body.value) {
    return jsonResponse({ error: "invalid_acme_challenge" }, 400);
  }

  const recordId = await upsertDnsRecord(env, {
    type: "TXT",
    name: expectedName,
    content: body.value,
  });
  await saveDevice(env, {
    ...device,
    txtRecordIds: [...new Set([...device.txtRecordIds, recordId])],
    updatedAt: new Date().toISOString(),
  });
  return jsonResponse({ recordId });
}

async function deleteAcmeTxt(
  request: Request,
  env: Env,
  device: DeviceRecord,
): Promise<Response> {
  const body = await readJson<{ recordId?: string }>(request);
  if (!body.recordId || !device.txtRecordIds.includes(body.recordId)) {
    return jsonResponse({ error: "unknown_txt_record" }, 404);
  }

  await cloudflareFetch(env, `/dns_records/${body.recordId}`, { method: "DELETE" });
  await saveDevice(env, {
    ...device,
    txtRecordIds: device.txtRecordIds.filter((id) => id !== body.recordId),
    updatedAt: new Date().toISOString(),
  });
  return jsonResponse({ ok: true });
}

async function authorizeDevice(
  request: Request,
  env: Env,
  deviceId: string,
): Promise<{ ok: true; record: DeviceRecord } | { ok: false; response: Response }> {
  const record = await env.DEVICES.get<DeviceRecord>(deviceKey(deviceId), "json");
  if (!record) {
    return { ok: false, response: jsonResponse({ error: "unknown_device" }, 404) };
  }

  const header = request.headers.get("authorization") || "";
  const secret = header.startsWith("Bearer ") ? header.slice(7).trim() : "";
  if (!secret || !timingSafeEqual(await hashSecret(secret), record.secretHash)) {
    return { ok: false, response: jsonResponse({ error: "unauthorized" }, 401) };
  }
  return { ok: true, record };
}

async function saveDevice(env: Env, record: DeviceRecord): Promise<void> {
  await env.DEVICES.put(deviceKey(record.id), JSON.stringify(record));
}

async function upsertDnsRecord(
  env: Env,
  input: { id?: string; type: "A" | "TXT"; name: string; content: string },
): Promise<string> {
  const payload = {
    type: input.type,
    name: input.name,
    content: input.content,
    ttl: 60,
    proxied: false,
  };
  const path = input.id ? `/dns_records/${input.id}` : "/dns_records";
  const method = input.id ? "PUT" : "POST";
  let result: CloudflareDnsRecord;
  try {
    result = await cloudflareFetch<CloudflareDnsRecord>(env, path, {
      method,
      body: JSON.stringify(payload),
    });
  } catch (error) {
    if (!input.id || !isMissingDnsRecordError(error)) {
      throw error;
    }
    result = await cloudflareFetch<CloudflareDnsRecord>(env, "/dns_records", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }
  return result.id;
}

function isMissingDnsRecordError(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return /Record does not exist|object identifier is invalid/i.test(error.message);
}

async function cloudflareFetch<T = unknown>(
  env: Env,
  path: string,
  init: RequestInit,
): Promise<T> {
  const response = await fetch(
    `https://api.cloudflare.com/client/v4/zones/${env.CLOUDFLARE_ZONE_ID}${path}`,
    {
      ...init,
      headers: {
        authorization: `Bearer ${env.CLOUDFLARE_API_TOKEN}`,
        "content-type": "application/json",
      },
    },
  );
  const parsed = (await response.json()) as CloudflareResponse<T>;
  if (!response.ok || !parsed.success || !parsed.result) {
    const message = parsed.errors?.map((error) => error.message).join(", ") || response.statusText;
    throw new Error(`cloudflare_api_error: ${message}`);
  }
  return parsed.result;
}

async function readJson<T>(request: Request): Promise<T> {
  const contentLength = Number(request.headers.get("content-length") || "0");
  if (contentLength > 32_768) {
    throw new Error("payload_too_large");
  }
  return (await request.json()) as T;
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: JSON_HEADERS,
  });
}

function deviceKey(id: string): string {
  return `device:${id}`;
}

function normalizeId(value?: string): string | undefined {
  const normalized = value?.trim().toLowerCase();
  return normalized && /^[a-z0-9-]{8,64}$/.test(normalized) ? normalized : undefined;
}

function normalizeLabel(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9-]/g, "").slice(0, 20) || "dmx";
}

function normalizeDomain(value: string): string {
  const domain = value.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/\/$/, "");
  if (!/^[a-z0-9.-]+$/.test(domain) || !domain.includes(".")) {
    throw new Error("invalid_bridge_domain");
  }
  return domain;
}

function normalizeAppUrl(value: string): string {
  const url = new URL(value);
  if (url.protocol !== "https:") {
    throw new Error("invalid_pwa_url");
  }
  return url.toString().replace(/\/$/, "");
}

function isPrivateIp(value: string): boolean {
  const parts = value.split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false;
  }
  const [a, b] = parts;
  return a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
}

async function hashSecret(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return base64Url(new Uint8Array(digest));
}

function randomBase64Url(length: number): string {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return base64Url(bytes);
}

function base64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

function timingSafeEqual(left: string, right: string): boolean {
  if (left.length !== right.length) {
    return false;
  }
  let diff = 0;
  for (let index = 0; index < left.length; index += 1) {
    diff |= left.charCodeAt(index) ^ right.charCodeAt(index);
  }
  return diff === 0;
}

function safeError(error: unknown): string {
  return error instanceof Error ? error.message : "unknown";
}
