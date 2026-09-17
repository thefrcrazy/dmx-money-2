const CACHE_NAME = "dmxmoney-shell-2.0.6";
const BUILD_ASSETS = [];
const APP_SHELL = [
  "/",
  "/mobile",
  "/mobile/",
  "/logo.png",
  "/manifest.webmanifest",
  "/pwa-192.png",
  "/pwa-512.png",
];

const isHttpRequest = (request) => {
  const url = new URL(request.url);
  return url.protocol === "http:" || url.protocol === "https:";
};

const putInCache = async (request, response) => {
  if (!response || !response.ok || !isHttpRequest(request)) return;
  const cache = await caches.open(CACHE_NAME);
  await cache.put(request, response.clone()).catch(() => undefined);
};

const networkFirst = async (request, fallbackPath) => {
  const cache = await caches.open(CACHE_NAME);
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 2500);
  try {
    const networkResponse = await fetch(request, { cache: "no-store", signal: controller.signal });
    if (networkResponse && networkResponse.ok) {
      await cache.put(request, networkResponse.clone()).catch(() => undefined);
      return networkResponse;
    }
  } catch {
    // Réseau indisponible ou hors-ligne
  } finally {
    clearTimeout(timeout);
  }

  const cached = (await cache.match(request))
    || (fallbackPath ? await cache.match(fallbackPath) : undefined);
  return cached || Response.error();
};

const staleWhileRevalidate = async (request, event) => {
  // Keep the refresh alive even when the cached response is returned immediately.
  const networkPromise = fetch(request)
    .then(async (response) => {
      await putInCache(request, response);
      return response;
    })
    .catch(() => undefined);
  event.waitUntil(networkPromise.then(() => undefined));
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request);
  return cached || (await networkPromise) || Response.error();
};

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll([...APP_SHELL, ...BUILD_ASSETS].map(url => new Request(url, { cache: "reload" }))))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(key => key.startsWith("dmxmoney-shell-") && key !== CACHE_NAME).map(key => caches.delete(key))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" || !isHttpRequest(request)) return;

  const url = new URL(request.url);
  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/auth/")) return;

  if (request.mode === "navigate") {
    event.respondWith(networkFirst(request, "/mobile"));
    return;
  }

  if (url.origin === self.location.origin) {
    event.respondWith(staleWhileRevalidate(request, event));
  }
});

