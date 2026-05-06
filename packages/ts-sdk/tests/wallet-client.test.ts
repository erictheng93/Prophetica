import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ApiError,
  createWalletClient,
  generateUuidV7,
  parseI64,
} from "../src/index.js";

test("parseI64 preserves server i64 values as bigint", () => {
  assert.equal(parseI64("9223372036854775807"), 9223372036854775807n);
  assert.equal(parseI64(-125000), -125000n);
});

test("wallet client reads balance i64 fields as bigint and sends auth context", async () => {
  const calls: Array<{ url: string; headers: Record<string, string> }> = [];
  const client = createWalletClient({
    baseUrl: "http://api.local/",
    auth: { userId: "user_42" },
    fetcher: async (input, init) => {
      calls.push({
        url: String(input),
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
      });

      return Response.json({
        user_id: "user_42",
        available_minor_units: "9223372036854775807",
        locked_minor_units: "-2500",
        frozen_minor_units: 0,
      });
    },
  });

  const balance = await client.getBalance();

  assert.equal(balance.userId, "user_42");
  assert.equal(balance.availableMinorUnits, 9223372036854775807n);
  assert.equal(balance.lockedMinorUnits, -2500n);
  assert.equal(balance.frozenMinorUnits, 0n);
  assert.equal(calls[0]?.url, "http://api.local/v1/me/wallet/balance");
  assert.equal(calls[0]?.headers["x-user-id"], "user_42");
});

test("wallet client posts transactions with generated UUID-v7 idempotency key", async () => {
  let requestBody: unknown;
  const client = createWalletClient({
    baseUrl: "http://api.local",
    auth: { bearerToken: "jwt" },
    fetcher: async (_input, init) => {
      requestBody = JSON.parse(String(init?.body));
      return Response.json({
        txn_id: "9007199254740993",
        idempotency_key: (requestBody as { idempotency_key: string }).idempotency_key,
        replayed: false,
      });
    },
  });

  const response = await client.postTransaction({
    txnType: "deposit",
    entries: [
      {
        accountCode: "Bank.TrustAccount.TWD",
        direction: "debit",
        amountMinorUnits: 10000n,
      },
      {
        accountCode: "Player.Wallet.user_42",
        direction: "credit",
        amountMinorUnits: 10000n,
      },
    ],
  });

  assert.match((requestBody as { idempotency_key: string }).idempotency_key, /^[0-9a-f-]{36}$/);
  assert.equal((requestBody as { idempotency_key: string }).idempotency_key[14], "7");
  assert.equal(response.txnId, 9007199254740993n);
  assert.equal(response.replayed, false);
});

test("fetch wrapper decodes ErrorEnvelope-compatible responses", async () => {
  const client = createWalletClient({
    baseUrl: "http://api.local",
    fetcher: async () =>
      Response.json(
        {
          code: "ERROR_CODE_LEDGER_VALIDATION_FAILED",
          message: "imbalanced transaction",
          trace_id: "trace-01",
          idempotency_key: "idem-01",
        },
        { status: 422 },
      ),
  });

  await assert.rejects(
    () => client.getBalance("user_42"),
    (error: unknown) => {
      assert.ok(error instanceof ApiError);
      assert.equal(error.status, 422);
      assert.equal(error.envelope.code, "ERROR_CODE_LEDGER_VALIDATION_FAILED");
      assert.equal(error.envelope.idempotencyKey, "idem-01");
      return true;
    },
  );
});

test("generateUuidV7 returns version 7 identifiers", () => {
  const id = generateUuidV7();

  assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
});
