import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KycSdkError,
  createMockKycProvider,
  createRealSumsubProvider,
  selectKycProvider,
  type KycInlineOutcome,
  type KycSessionEvent,
} from "../src/kyc-provider/index.js";

const fakeTarget = (): HTMLElement => ({}) as unknown as HTMLElement;

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

test("mock provider emits ready then submitted via auto-submit", async () => {
  const provider = createMockKycProvider({ autoSubmitAfterMs: 10 });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "zh-TW",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  await wait(40);

  assert.equal(events[0]?.kind, "ready");
  assert.ok(
    events.some((e) => e.kind === "submitted"),
    "expected a submitted event before deadline",
  );

  handle.unmount();
});

test("mock provider unmount stops further events and is idempotent", async () => {
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 50,
    globalHookKey: "__mockKycComplete_idempotent",
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "en",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  handle.unmount();
  handle.unmount();
  await wait(80);

  assert.ok(
    events.every((e) => e.kind !== "submitted"),
    "submitted must not fire after unmount",
  );
});

test("unsubscribe stops the listener from receiving further events", async () => {
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 20,
    globalHookKey: "__mockKycComplete_unsub",
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "en",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  const unsubscribe = handle.subscribe((event) => events.push(event));
  unsubscribe();
  await wait(60);

  assert.ok(
    events.every((e) => e.kind !== "submitted"),
    "submitted must not reach the listener after unsubscribe",
  );

  handle.unmount();
});

test("global hook emits a completed event with the supplied outcome", async () => {
  const hookKey = "__mockKycComplete_global";
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 0,
    globalHookKey: hookKey,
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_3",
    locale: "ja-JP",
    country: "JP",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  await wait(5);

  type Hook = (outcome: KycInlineOutcome) => void;
  const host = globalThis as Record<string, unknown>;
  const hook = host[hookKey] as Hook | undefined;
  assert.equal(typeof hook, "function", "global hook should be installed");
  hook?.("approved");

  const completed = events.find((e) => e.kind === "completed");
  assert.equal(completed?.kind, "completed");
  if (completed?.kind === "completed") {
    assert.equal(completed.outcome, "approved");
  }

  handle.unmount();

  assert.equal(
    host[hookKey],
    undefined,
    "global hook should be removed after unmount",
  );
});

test("selectKycProvider resolves provider by sdk token prefix", () => {
  const sandbox = selectKycProvider({
    sdkToken: "sumsub_sandbox_user_42_TIER_2_1",
  });
  assert.equal(sandbox.name, "mock");

  const real = selectKycProvider({ sdkToken: "sumsub_real_token_xyz" });
  assert.equal(real.name, "sumsub");

  const mock = selectKycProvider({ sdkToken: "mock_local_dev" });
  assert.equal(mock.name, "mock");
});

test("selectKycProvider throws KycSdkError on unknown prefix", () => {
  assert.throws(
    () => selectKycProvider({ sdkToken: "onfido_xxx" }),
    (err: unknown) =>
      err instanceof KycSdkError && err.code === "unknown_sdk_token_prefix",
  );
});

test("real SumSub provider mount throws not_implemented", async () => {
  const provider = createRealSumsubProvider();
  await assert.rejects(
    () =>
      provider.mount(fakeTarget(), {
        sdkToken: "sumsub_real_xyz",
        targetTier: "TIER_2",
        locale: "en",
        country: "TW",
      }),
    (err: unknown) =>
      err instanceof KycSdkError && err.code === "not_implemented",
  );
});

test("override option short-circuits sdk token resolution", () => {
  const stub = createMockKycProvider();
  const resolved = selectKycProvider({
    sdkToken: "anything_unsupported",
    override: stub,
  });
  assert.strictEqual(resolved, stub);
});
