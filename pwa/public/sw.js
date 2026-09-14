const CACHE_NAME = 'dmxmoney-shell-v32';
const APP_SHELL = [
  '/',
  '/mobile',
  '/logo.png',
  '/manifest.webmanifest',
  '/pwa-192.png',
  '/pwa-512.png',
];

const isHttpRequest = (request) => {
  const url = new URL(request.url);
  return url.protocol === 'http:' || url.protocol === 'https:';
};

const putInCache = async (request, response) => {
  if (!response || !response.ok || !isHttpRequest(request)) return;
  const cache = await caches.open(CACHE_NAME);
  await cache.put(request, response.clone());
};

const networkFirst = async (request, fallbackPath) => {
  try {
    const response = await fetch(request);
    await putInCache(request, response);
    return response;
  } catch {
    const cache = await caches.open(CACHE_NAME);
    return (await cache.match(request))
      || (fallbackPath ? await cache.match(fallbackPath) : undefined)
      || Response.error();
  }
};

const staleWhileRevalidate = async (request) => {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(request);
  const network = fetch(request)
    .then(async (response) => {
      await putInCache(request, response);
      return response;
    })
    .catch(() => undefined);

  return cached || await network || Response.error();
};

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(APP_SHELL))
      .catch(() => undefined)
      .then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(key => key !== CACHE_NAME).map(key => caches.delete(key))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET' || !isHttpRequest(request)) return;

  const url = new URL(request.url);
  if (url.pathname.startsWith('/api/')) return;

  if (request.mode === 'navigate') {
    event.respondWith(networkFirst(request, '/mobile'));
    return;
  }

  if (url.origin === self.location.origin) {
    event.respondWith(staleWhileRevalidate(request));
  }
});
