/**
 * RVF Seed Decoder - Service Worker
 *
 * Cache-first strategy for static assets and the WASM binary.
 * Falls back to network on cache miss, then caches the response.
 */

const CACHE_NAME = 'rvf-pwa-v1';

const STATIC_ASSETS = [
  './',
  './index.html',
  './app.js',
  './style.css',
  './manifest.json',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(STATIC_ASSETS);
    })
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((names) => {
      return Promise.all(
        names
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      );
    })
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // Cache-first for same-origin static assets and WASM binaries
  if (url.origin === self.location.origin) {
    event.respondWith(
      caches.open(CACHE_NAME).then((cache) => {
        return cache.match(event.request).then((cached) => {
          if (cached) {
            return cached;
          }
          return fetch(event.request).then((response) => {
            // Cache successful GET responses
            if (response.ok && event.request.method === 'GET') {
              const isWasm = url.pathname.endsWith('.wasm');
              const isStatic =
                url.pathname.endsWith('.html') ||
                url.pathname.endsWith('.js') ||
                url.pathname.endsWith('.css') ||
                url.pathname.endsWith('.json');

              if (isWasm || isStatic) {
                cache.put(event.request, response.clone());
              }
            }
            return response;
          });
        });
      }).catch(() => {
        // Offline fallback: return cached index for navigation requests
        if (event.request.mode === 'navigate') {
          return caches.match('./index.html');
        }
        return new Response('Offline', { status: 503 });
      })
    );
  }
});
