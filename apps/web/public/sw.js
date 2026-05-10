/*
 * Prophetica web service worker — Phase A (SPEC-017 §6 amend, 2026-05-10).
 *
 * Caching strategy
 *   - Static assets (`/_next/static/...`, fonts, icons): cache-first.
 *     They are content-hashed by Next.js, so a stale entry can never
 *     diverge from the deployed version; cache hits give offline parity.
 *   - HTML navigations: network-first with an /offline fallback. The
 *     network response is also written back to runtime cache so a fresh
 *     install can survive an offline reload of any visited page.
 *   - JSON / API GETs: stale-while-revalidate. Returns cached copy
 *     immediately, fires a background fetch, replaces the entry. Read
 *     surfaces feel snappy without sacrificing freshness.
 *   - POST / PUT / DELETE: pass-through. We deliberately do NOT queue
 *     mutating requests; SPEC-002 funds-safety forbids silent retries
 *     of ledger writes. Background sync is parked in TODO.md LG-8 and
 *     must be co-designed with the api-gateway idempotency contract.
 *
 * Cache versioning
 *   STATIC_CACHE / RUNTIME_CACHE are tagged with the build hash; bumping
 *   either string evicts the corresponding entries on activation.
 */

const VERSION = "v1";
const STATIC_CACHE = `proph-static-${VERSION}`;
const RUNTIME_CACHE = `proph-runtime-${VERSION}`;
const OFFLINE_URL = "/offline";

const PRECACHE_URLS = [OFFLINE_URL, "/manifest.webmanifest"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(STATIC_CACHE);
      // Add() per-URL so a single 404 (e.g. /offline not yet built)
      // does not abort the entire install — the SW still installs and
      // the missing entry is filled by the first navigation that hits it.
      await Promise.all(
        PRECACHE_URLS.map((url) =>
          cache.add(url).catch((err) => {
            console.warn(`[sw] precache miss ${url}`, err);
          }),
        ),
      );
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys
          .filter((key) => key !== STATIC_CACHE && key !== RUNTIME_CACHE)
          .map((key) => caches.delete(key)),
      );
      await self.clients.claim();
    })(),
  );
});

function isStaticAsset(url) {
  return (
    url.pathname.startsWith("/_next/static/") ||
    url.pathname.startsWith("/icon") ||
    url.pathname === "/apple-icon" ||
    url.pathname.endsWith(".woff2") ||
    url.pathname.endsWith(".woff") ||
    url.pathname.endsWith(".ttf")
  );
}

function isApiGet(request, url) {
  return request.method === "GET" && url.pathname.startsWith("/api/");
}

async function networkFirstHtml(request) {
  const cache = await caches.open(RUNTIME_CACHE);
  try {
    const network = await fetch(request);
    if (network.ok) {
      cache.put(request, network.clone());
    }
    return network;
  } catch {
    const cached = await cache.match(request);
    if (cached) return cached;
    const offline = await caches.match(OFFLINE_URL);
    return offline ?? new Response("Offline", { status: 503, statusText: "Offline" });
  }
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  const network = await fetch(request);
  if (network.ok) {
    const cache = await caches.open(STATIC_CACHE);
    cache.put(request, network.clone());
  }
  return network;
}

async function staleWhileRevalidate(request) {
  const cache = await caches.open(RUNTIME_CACHE);
  const cached = await cache.match(request);
  const networkPromise = fetch(request)
    .then((response) => {
      if (response.ok) cache.put(request, response.clone());
      return response;
    })
    .catch(() => undefined);
  return cached ?? (await networkPromise) ?? new Response("Offline", { status: 503 });
}

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Cross-origin requests bypass the SW entirely.
  if (url.origin !== self.location.origin) return;

  // Mutating verbs always go straight to the network — see file header.
  if (request.method !== "GET") return;

  if (request.mode === "navigate" || request.headers.get("accept")?.includes("text/html")) {
    event.respondWith(networkFirstHtml(request));
    return;
  }

  if (isStaticAsset(url)) {
    event.respondWith(cacheFirst(request));
    return;
  }

  if (isApiGet(request, url)) {
    event.respondWith(staleWhileRevalidate(request));
  }
});
