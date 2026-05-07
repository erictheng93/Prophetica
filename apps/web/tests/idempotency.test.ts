import assert from "node:assert/strict";
import { test } from "node:test";

import {
  generateIdempotencyKey,
  IDEMPOTENCY_HEADER,
  uuidV7Timestamp,
  withIdempotencyKey,
} from "../lib/idempotency.js";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

test("generateIdempotencyKey emits a UUID v7 with version + variant nibbles", () => {
  const key = generateIdempotencyKey();
  assert.match(key, UUID_RE);
});

test("generateIdempotencyKey embeds the millisecond timestamp", () => {
  const fixed = 1_750_000_000_000;
  const key = generateIdempotencyKey(fixed);
  assert.equal(uuidV7Timestamp(key), fixed);
});

test("generateIdempotencyKey is unique across rapid calls (1000 keys, no collisions)", () => {
  const keys = new Set<string>();
  for (let i = 0; i < 1000; i += 1) {
    keys.add(generateIdempotencyKey());
  }
  assert.equal(keys.size, 1000);
});

test("withIdempotencyKey sets the canonical header and surfaces the key", () => {
  const init = withIdempotencyKey({ method: "POST" });
  const headers = new Headers(init.headers);

  assert.equal(headers.get(IDEMPOTENCY_HEADER), init.idempotencyKey);
  assert.match(init.idempotencyKey, UUID_RE);
  assert.equal(init.method, "POST");
});

test("withIdempotencyKey reuses caller-supplied key for retry replay", () => {
  const explicitKey = "01900000-0000-7000-8000-000000000000";
  const init = withIdempotencyKey({ idempotencyKey: explicitKey });

  assert.equal(init.idempotencyKey, explicitKey);
  assert.equal(new Headers(init.headers).get(IDEMPOTENCY_HEADER), explicitKey);
});

test("uuidV7Timestamp rejects non-v7 values", () => {
  assert.equal(uuidV7Timestamp("01900000-0000-4000-8000-000000000000"), null);
  assert.equal(uuidV7Timestamp("not-a-uuid"), null);
});
