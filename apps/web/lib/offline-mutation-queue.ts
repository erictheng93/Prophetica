export const IDEMPOTENCY_HEADER = "Idempotency-Key";

export type OfflineMutationMethod = "POST" | "DELETE";
export type OfflineMutationReconciliation = "engine-order-status";

export interface OfflineMutationPolicy {
  id: string;
  methods: OfflineMutationMethod[];
  pathPattern: string;
  reconciliation: OfflineMutationReconciliation;
}

export const OFFLINE_MUTATION_POLICIES: ReadonlyArray<OfflineMutationPolicy> = [
  {
    id: "trading-order-placement",
    methods: ["POST"],
    pathPattern: "^/v1/markets/[^/]+/orders$",
    reconciliation: "engine-order-status",
  },
  {
    id: "trading-order-placement-bff",
    methods: ["POST"],
    pathPattern: "^/api/engine/markets/[^/]+/orders$",
    reconciliation: "engine-order-status",
  },
  {
    id: "trading-order-cancellation",
    methods: ["DELETE"],
    pathPattern: "^/v1/orders/[^/]+$",
    reconciliation: "engine-order-status",
  },
];

export interface OfflineMutationItem {
  id: string;
  url: string;
  method: OfflineMutationMethod;
  headers: Array<[string, string]>;
  body: string | null;
  idempotencyKey: string;
  createdAt: number;
  attempts: number;
}

export interface OfflineMutationStore {
  list(): Promise<OfflineMutationItem[]>;
  put(item: OfflineMutationItem): Promise<void>;
  delete(id: string): Promise<void>;
}

export type Fetcher = (
  input: string | URL,
  init?: RequestInit,
) => Promise<Response>;

export interface OfflineMutationQueueOptions {
  store?: OfflineMutationStore;
  now?: () => number;
}

export interface DrainOptions {
  fetcher?: Fetcher;
}

export interface DrainResult {
  replayed: number;
  failed: number;
}

export function findOfflineMutationPolicy(
  request: Request,
): OfflineMutationPolicy | null {
  const url = new URL(request.url);
  const method = request.method as OfflineMutationMethod;
  return (
    OFFLINE_MUTATION_POLICIES.find(
      (policy) =>
        policy.methods.includes(method) &&
        new RegExp(policy.pathPattern).test(url.pathname),
    ) ?? null
  );
}

export function isQueueableMutation(request: Request): boolean {
  return findOfflineMutationPolicy(request) !== null;
}

export function isQueueableTradingMutation(request: Request): boolean {
  return isQueueableMutation(request);
}

export function createOfflineMutationQueue(
  options: OfflineMutationQueueOptions = {},
) {
  const store = options.store ?? new IndexedDbOfflineMutationStore();
  const now = options.now ?? Date.now;

  return {
    async enqueueRequest(request: Request): Promise<OfflineMutationItem> {
      if (!isQueueableMutation(request)) {
        throw new Error("request is not queueable");
      }

      const idempotencyKey = request.headers.get(IDEMPOTENCY_HEADER)?.trim();
      if (!idempotencyKey) {
        throw new Error("queueable trading mutations require Idempotency-Key");
      }

      const method = request.method as OfflineMutationMethod;
      const body = method === "POST" ? await request.clone().text() : null;
      const item: OfflineMutationItem = {
        id: `${method}:${new URL(request.url).pathname}:${idempotencyKey}`,
        url: request.url,
        method,
        headers: Array.from(request.headers.entries()),
        body,
        idempotencyKey,
        createdAt: now(),
        attempts: 0,
      };
      await store.put(item);
      return item;
    },

    async drain(options: DrainOptions = {}): Promise<DrainResult> {
      const fetcher = options.fetcher ?? fetch;
      const items = (await store.list()).sort(
        (a, b) => a.createdAt - b.createdAt,
      );
      let replayed = 0;
      let failed = 0;

      for (const item of items) {
        const replay = await fetcher(item.url, {
          method: item.method,
          cache: "no-store",
          headers: new Headers(item.headers),
          body: item.body ?? undefined,
        }).catch(() => undefined);

        if (replay?.ok) {
          await store.delete(item.id);
          replayed += 1;
        } else {
          await store.put({ ...item, attempts: item.attempts + 1 });
          failed += 1;
        }
      }

      return { replayed, failed };
    },
  };
}

export class MemoryOfflineMutationStore implements OfflineMutationStore {
  private readonly items = new Map<string, OfflineMutationItem>();

  async list(): Promise<OfflineMutationItem[]> {
    return Array.from(this.items.values(), cloneItem);
  }

  async put(item: OfflineMutationItem): Promise<void> {
    this.items.set(item.id, cloneItem(item));
  }

  async delete(id: string): Promise<void> {
    this.items.delete(id);
  }
}

export class IndexedDbOfflineMutationStore implements OfflineMutationStore {
  private readonly dbName: string;
  private readonly storeName: string;

  constructor(
    dbName = "prophetica-offline-mutations",
    storeName = "mutations",
  ) {
    this.dbName = dbName;
    this.storeName = storeName;
  }

  async list(): Promise<OfflineMutationItem[]> {
    const store = await this.openStore("readonly");
    return new Promise((resolve, reject) => {
      const request = store.getAll();
      request.onsuccess = () =>
        resolve((request.result as OfflineMutationItem[]).map(cloneItem));
      request.onerror = () => reject(request.error);
    });
  }

  async put(item: OfflineMutationItem): Promise<void> {
    const store = await this.openStore("readwrite");
    await requestDone(store.put(cloneItem(item)));
  }

  async delete(id: string): Promise<void> {
    const store = await this.openStore("readwrite");
    await requestDone(store.delete(id));
  }

  private async openStore(mode: IDBTransactionMode): Promise<IDBObjectStore> {
    const db = await openDatabase(this.dbName, this.storeName);
    return db.transaction(this.storeName, mode).objectStore(this.storeName);
  }
}

function cloneItem(item: OfflineMutationItem): OfflineMutationItem {
  return {
    ...item,
    headers: item.headers.map(([name, value]) => [name, value]),
  };
}

function openDatabase(dbName: string, storeName: string): Promise<IDBDatabase> {
  if (!globalThis.indexedDB) {
    return Promise.reject(new Error("IndexedDB is not available"));
  }

  return new Promise((resolve, reject) => {
    const request = globalThis.indexedDB.open(dbName, 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(storeName)) {
        db.createObjectStore(storeName, { keyPath: "id" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function requestDone<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
