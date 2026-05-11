import assert from "node:assert/strict";
import { test } from "node:test";

import {
  IDEMPOTENCY_HEADER,
  createOfflineMutationQueue,
  isQueueableTradingMutation,
  MemoryOfflineMutationStore,
} from "../lib/offline-mutation-queue.js";

test("only order placement and cancellation mutations are queueable", () => {
  assert.equal(
    isQueueableTradingMutation(
      new Request("https://app.local/v1/markets/mkt-election/orders", {
        method: "POST",
      }),
    ),
    true,
  );
  assert.equal(
    isQueueableTradingMutation(
      new Request(
        "https://app.local/v1/orders/ord_01HZ0000000000000000000000",
        {
          method: "DELETE",
        },
      ),
    ),
    true,
  );
  assert.equal(
    isQueueableTradingMutation(
      new Request("https://app.local/internal/ledger/transactions", {
        method: "POST",
      }),
    ),
    false,
  );
  assert.equal(
    isQueueableTradingMutation(
      new Request("https://app.local/v1/markets/mkt-election/orders", {
        method: "GET",
      }),
    ),
    false,
  );
});

test("enqueueRequest persists trading mutation with original idempotency key", async () => {
  const store = new MemoryOfflineMutationStore();
  const queue = createOfflineMutationQueue({
    store,
    now: () => 1_800_000_000_000,
  });
  const request = new Request(
    "https://app.local/v1/markets/mkt-election/orders",
    {
      method: "POST",
      headers: {
        "content-type": "application/json",
        [IDEMPOTENCY_HEADER]: "idem-order-001",
        "x-user-id": "10",
      },
      body: JSON.stringify({ side: "sell", quantity: 3 }),
    },
  );

  const item = await queue.enqueueRequest(request);
  const stored = await store.list();

  assert.equal(item.idempotencyKey, "idem-order-001");
  assert.equal(stored.length, 1);
  assert.equal(stored[0]?.method, "POST");
  assert.equal(
    stored[0]?.url,
    "https://app.local/v1/markets/mkt-election/orders",
  );
  assert.equal(stored[0]?.body, '{"side":"sell","quantity":3}');
});

test("drain replays queued mutation with original Idempotency-Key and removes success", async () => {
  const store = new MemoryOfflineMutationStore();
  const queue = createOfflineMutationQueue({
    store,
    now: () => 1_800_000_000_000,
  });
  await queue.enqueueRequest(
    new Request("https://app.local/v1/orders/ord_01HZ0000000000000000000000", {
      method: "DELETE",
      headers: {
        [IDEMPOTENCY_HEADER]: "idem-cancel-001",
        "x-user-id": "10",
      },
    }),
  );
  const calls: Array<{ url: string; headers: Headers; method?: string }> = [];

  const result = await queue.drain({
    fetcher: async (input, init) => {
      calls.push({
        url: String(input),
        headers: new Headers(init?.headers),
        method: init?.method,
      });
      return Response.json({ status: "CANCELLED" });
    },
  });

  assert.equal(result.replayed, 1);
  assert.equal(result.failed, 0);
  assert.equal(calls[0]?.method, "DELETE");
  assert.equal(calls[0]?.headers.get(IDEMPOTENCY_HEADER), "idem-cancel-001");
  assert.equal((await store.list()).length, 0);
});

test("drain keeps failed mutation for a later replay", async () => {
  const store = new MemoryOfflineMutationStore();
  const queue = createOfflineMutationQueue({
    store,
    now: () => 1_800_000_000_000,
  });
  await queue.enqueueRequest(
    new Request("https://app.local/v1/markets/mkt-election/orders", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        [IDEMPOTENCY_HEADER]: "idem-order-002",
      },
      body: "{}",
    }),
  );

  const result = await queue.drain({
    fetcher: async () => new Response("unavailable", { status: 503 }),
  });

  const stored = await store.list();
  assert.equal(result.replayed, 0);
  assert.equal(result.failed, 1);
  assert.equal(stored.length, 1);
  assert.equal(stored[0]?.attempts, 1);
});
