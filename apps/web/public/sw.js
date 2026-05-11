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
 *   - Mutations: only the api-gateway trading commands listed in
 *     isQueueableTradingMutation() can be queued, and only when they
 *     carry Idempotency-Key. All other POST / PUT / DELETE requests are
 *     pass-through per SPEC-002 funds-safety.
 *
 * Cache versioning
 *   STATIC_CACHE / RUNTIME_CACHE are tagged with the build hash; bumping
 *   either string evicts the corresponding entries on activation.
 */

const VERSION = "v1";
const STATIC_CACHE = `proph-static-${VERSION}`;
const RUNTIME_CACHE = `proph-runtime-${VERSION}`;
const OFFLINE_URL = "/offline";
const OFFLINE_MUTATION_DB = "prophetica-offline-mutations";
const OFFLINE_MUTATION_STORE = "mutations";
const OFFLINE_SYNC_TAG = "proph-offline-mutations";
const IDEMPOTENCY_HEADER = "Idempotency-Key";

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
    url.pathname.startsWith("/icon-") ||
    url.pathname === "/favicon-32.png" ||
    url.pathname === "/apple-touch-icon.png" ||
    url.pathname.endsWith(".woff2") ||
    url.pathname.endsWith(".woff") ||
    url.pathname.endsWith(".ttf")
  );
}

function isApiGet(request, url) {
  return request.method === "GET" && url.pathname.startsWith("/api/");
}

function isQueueableTradingMutation(request, url) {
  if (request.method === "POST") {
    return /^\/v1\/markets\/[^/]+\/orders$/.test(url.pathname);
  }
  if (request.method === "DELETE") {
    return /^\/v1\/orders\/[^/]+$/.test(url.pathname);
  }
  return false;
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
    return (
      offline ?? new Response("Offline", { status: 503, statusText: "Offline" })
    );
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
  return (
    cached ?? (await networkPromise) ?? new Response("Offline", { status: 503 })
  );
}

async function fetchOrQueueTradingMutation(request) {
  try {
    return await fetch(request.clone());
  } catch {
    const idempotencyKey = request.headers.get(IDEMPOTENCY_HEADER)?.trim();
    if (!idempotencyKey) {
      return Response.json(
        {
          error: "idempotency_key_required",
          message: "Offline trading mutations require Idempotency-Key.",
        },
        { status: 400 },
      );
    }

    const item = await mutationItemFromRequest(request, idempotencyKey);
    await putOfflineMutation(item);
    await registerOfflineSync();

    return Response.json(
      {
        status: "queued",
        idempotency_key: idempotencyKey,
        queued_at: item.createdAt,
      },
      { status: 202 },
    );
  }
}

async function mutationItemFromRequest(request, idempotencyKey) {
  const url = new URL(request.url);
  const body = request.method === "POST" ? await request.clone().text() : null;
  return {
    id: `${request.method}:${url.pathname}:${idempotencyKey}`,
    url: request.url,
    method: request.method,
    headers: Array.from(request.headers.entries()),
    body,
    idempotencyKey,
    createdAt: Date.now(),
    attempts: 0,
  };
}

async function registerOfflineSync() {
  if (typeof self.registration?.sync?.register === "function") {
    await self.registration.sync
      .register(OFFLINE_SYNC_TAG)
      .catch(() => undefined);
  }
}

async function drainOfflineMutations() {
  const items = await listOfflineMutations();
  items.sort((a, b) => a.createdAt - b.createdAt);

  for (const item of items) {
    const response = await fetch(item.url, {
      method: item.method,
      cache: "no-store",
      headers: new Headers(item.headers),
      body: item.body ?? undefined,
    }).catch(() => undefined);

    if (response?.ok) {
      await deleteOfflineMutation(item.id);
    } else {
      await putOfflineMutation({ ...item, attempts: item.attempts + 1 });
    }
  }
}

function openOfflineMutationDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(OFFLINE_MUTATION_DB, 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(OFFLINE_MUTATION_STORE)) {
        db.createObjectStore(OFFLINE_MUTATION_STORE, { keyPath: "id" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function offlineMutationStore(mode) {
  const db = await openOfflineMutationDb();
  return db
    .transaction(OFFLINE_MUTATION_STORE, mode)
    .objectStore(OFFLINE_MUTATION_STORE);
}

function requestDone(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function listOfflineMutations() {
  const store = await offlineMutationStore("readonly");
  return requestDone(store.getAll());
}

async function putOfflineMutation(item) {
  const store = await offlineMutationStore("readwrite");
  await requestDone(store.put(item));
}

async function deleteOfflineMutation(id) {
  const store = await offlineMutationStore("readwrite");
  await requestDone(store.delete(id));
}

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Cross-origin requests bypass the SW entirely.
  if (url.origin !== self.location.origin) return;

  if (request.method !== "GET") {
    if (isQueueableTradingMutation(request, url)) {
      event.respondWith(fetchOrQueueTradingMutation(request));
    }
    return;
  }

  if (
    request.mode === "navigate" ||
    request.headers.get("accept")?.includes("text/html")
  ) {
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

self.addEventListener("sync", (event) => {
  if (event.tag === OFFLINE_SYNC_TAG) {
    event.waitUntil(drainOfflineMutations());
  }
});

self.addEventListener("message", (event) => {
  if (event.data?.type === "DRAIN_OFFLINE_MUTATIONS") {
    event.waitUntil(drainOfflineMutations());
  }
});
