import assert from "node:assert/strict";
import { test } from "node:test";

import {
  initialKycPhase,
  isPollingTimeoutPhase,
  isRetryablePhase,
  isTerminalPhase,
  reduceKycPhase,
  type KycPhase,
} from "../lib/kyc-state-machine.js";
import { buildKycIdempotencyKey } from "../lib/kyc-idempotency.js";
import {
  createKycWebhookPoller,
  pollingActionForKycStatus,
} from "../lib/kyc-webhook-polling.js";

test("idle -> initializing on start", () => {
  const next = reduceKycPhase(initialKycPhase, { type: "start" });
  assert.equal(next.kind, "initializing");
});

test("upgrade_failed lands in recoverable error from initializing", () => {
  const next = reduceKycPhase(
    { kind: "initializing" },
    { type: "upgrade_failed" },
  );
  assert.equal(next.kind, "error");
  if (next.kind === "error") {
    assert.equal(next.code, "start_upgrade_failed");
    assert.equal(next.recoverable, true);
  }
});

test("mount_failed marks browser_unsupported as non-recoverable", () => {
  const next = reduceKycPhase(
    { kind: "mounting" },
    { type: "mount_failed", code: "browser_unsupported" },
  );
  assert.equal(next.kind, "error");
  if (next.kind === "error") {
    assert.equal(next.recoverable, false);
  }
});

test("mount_failed marks provider_unavailable as non-recoverable", () => {
  const next = reduceKycPhase(
    { kind: "mounting" },
    { type: "mount_failed", code: "provider_unavailable" },
  );
  assert.equal(next.kind, "error");
  if (next.kind === "error") {
    assert.equal(next.code, "provider_unavailable");
    assert.equal(next.recoverable, false);
  }
});

test("submitted event from in_progress -> awaiting_webhook with timestamp", () => {
  const t = 1_700_000_000_000;
  const next = reduceKycPhase(
    { kind: "in_progress" },
    { type: "sdk_event", event: { kind: "submitted" }, nowMs: t },
  );
  assert.equal(next.kind, "awaiting_webhook");
  if (next.kind === "awaiting_webhook") {
    assert.equal(next.submittedAtMs, t);
  }
});

test("submitted event for a continuing stage advances through mounting for the next stage", () => {
  const submitted = reduceKycPhase(
    { kind: "in_progress" },
    {
      type: "sdk_event",
      event: { kind: "submitted" },
      nowMs: 1_700_000_000_000,
      continueAfterSubmit: true,
    },
  );

  assert.equal(submitted.kind, "awaiting_stage_complete");

  const nextStage = reduceKycPhase(submitted, { type: "stage_advance" });
  assert.equal(nextStage.kind, "mounting");
});

test("polling_timeout from awaiting_webhook -> recoverable error", () => {
  const next = reduceKycPhase(
    { kind: "awaiting_webhook", submittedAtMs: 0 },
    { type: "polling_timeout" },
  );
  assert.equal(next.kind, "error");
  if (next.kind === "error") {
    assert.equal(next.code, "polling_timeout");
    assert.equal(next.recoverable, true);
  }
});

test("retry from terminal phases collapses back to idle", () => {
  for (const phase of [
    { kind: "abandoned" } as const,
    { kind: "failed", reason: "rejected" } as const,
    { kind: "error", code: "network_lost", recoverable: true } as const,
  ]) {
    assert.equal(reduceKycPhase(phase, { type: "retry" }).kind, "idle");
  }
});

test("retry is a no-op from non-terminal phases", () => {
  for (const phase of [
    { kind: "initializing" } as const,
    { kind: "mounting" } as const,
    { kind: "in_progress" } as const,
  ] satisfies KycPhase[]) {
    assert.deepEqual(reduceKycPhase(phase, { type: "retry" }), phase);
  }
});

test("status_passed from awaiting_webhook -> done; status_failed -> failed", () => {
  const passed = reduceKycPhase(
    { kind: "awaiting_webhook", submittedAtMs: 0 },
    { type: "status_passed" },
  );
  assert.equal(passed.kind, "done");

  const failed = reduceKycPhase(
    { kind: "awaiting_webhook", submittedAtMs: 0 },
    { type: "status_failed", reason: "rejected" },
  );
  assert.equal(failed.kind, "failed");
});

test("webhook polling maps completed backend statuses into phase actions", () => {
  assert.deepEqual(pollingActionForKycStatus("passed"), {
    type: "status_passed",
  });
  assert.deepEqual(pollingActionForKycStatus("rejected"), {
    type: "status_failed",
    reason: "rejected",
  });
  assert.deepEqual(pollingActionForKycStatus("frozen"), {
    type: "status_failed",
    reason: "frozen",
  });
  assert.deepEqual(pollingActionForKycStatus("expired"), {
    type: "status_failed",
    reason: "expired",
  });
  assert.equal(pollingActionForKycStatus("pending"), null);
});

test("webhook poller dispatches terminal backend status and stops polling", async () => {
  const timers = createFakeTimers();
  const actions: unknown[] = [];

  createKycWebhookPoller({
    submittedAtMs: 1_000,
    refreshKycOverview: async () => ({ status: "passed" }),
    dispatch: (action) => actions.push(action),
    now: () => 2_000,
    setTimeout: timers.setTimeout,
    clearTimeout: timers.clearTimeout,
  });

  assert.equal(timers.pendingCount(), 1);
  await timers.runNext();

  assert.deepEqual(actions, [{ type: "status_passed" }]);
  assert.equal(timers.pendingCount(), 0);
});

test("webhook poller times out and cleanup cancels pending tick", async () => {
  const timeoutTimers = createFakeTimers();
  const timeoutActions: unknown[] = [];

  createKycWebhookPoller({
    submittedAtMs: 0,
    refreshKycOverview: async () => ({ status: "pending" }),
    dispatch: (action) => timeoutActions.push(action),
    now: () => 301_000,
    setTimeout: timeoutTimers.setTimeout,
    clearTimeout: timeoutTimers.clearTimeout,
  });
  await timeoutTimers.runNext();
  assert.deepEqual(timeoutActions, [{ type: "polling_timeout" }]);

  const cleanupTimers = createFakeTimers();
  const poller = createKycWebhookPoller({
    submittedAtMs: 1_000,
    refreshKycOverview: async () => ({ status: "pending" }),
    dispatch: () => {},
    now: () => 2_000,
    setTimeout: cleanupTimers.setTimeout,
    clearTimeout: cleanupTimers.clearTimeout,
  });
  assert.equal(cleanupTimers.pendingCount(), 1);
  poller.stop();
  assert.equal(cleanupTimers.pendingCount(), 0);
});

test("isTerminalPhase / isRetryablePhase classifications", () => {
  assert.equal(isTerminalPhase({ kind: "done" }), true);
  assert.equal(isTerminalPhase({ kind: "failed", reason: "rejected" }), true);
  assert.equal(isTerminalPhase({ kind: "abandoned" }), true);
  assert.equal(
    isTerminalPhase({
      kind: "error",
      code: "network_lost",
      recoverable: true,
    }),
    true,
  );
  assert.equal(isTerminalPhase({ kind: "in_progress" }), false);

  assert.equal(isRetryablePhase({ kind: "abandoned" }), true);
  assert.equal(
    isRetryablePhase({
      kind: "error",
      code: "browser_unsupported",
      recoverable: false,
    }),
    false,
  );
  assert.equal(
    isRetryablePhase({
      kind: "error",
      code: "network_lost",
      recoverable: true,
    }),
    true,
  );
});

test("idempotency key has 3 segments on first attempt, 4 on retry", () => {
  const first = buildKycIdempotencyKey({
    userId: "user_42",
    targetTier: "TIER_2",
  });
  assert.equal(first, "kyc:user_42:TIER_2");

  const retry = buildKycIdempotencyKey({
    userId: "user_42",
    targetTier: "TIER_2",
    retryAttempt: 3,
  });
  assert.equal(retry, "kyc:user_42:TIER_2:retry3");
});

test("idempotency key dedupes double-clicks: same input -> same key", () => {
  const a = buildKycIdempotencyKey({ userId: "u", targetTier: "TIER_2" });
  const b = buildKycIdempotencyKey({ userId: "u", targetTier: "TIER_2" });
  assert.equal(a, b);
});

test("polling_timeout is not retryable even though it is recoverable", () => {
  const phase: KycPhase = {
    kind: "error",
    code: "polling_timeout",
    recoverable: true,
  };
  assert.equal(isPollingTimeoutPhase(phase), true);
  assert.equal(isRetryablePhase(phase), false);
});

test("isPollingTimeoutPhase distinguishes from other recoverable errors", () => {
  const networkLoss: KycPhase = {
    kind: "error",
    code: "network_lost",
    recoverable: true,
  };
  assert.equal(isPollingTimeoutPhase(networkLoss), false);
  assert.equal(isRetryablePhase(networkLoss), true);
});

function createFakeTimers() {
  type Timer = {
    id: number;
    callback: () => void | Promise<void>;
  };
  const pending = new Map<number, Timer>();
  let nextId = 1;

  return {
    setTimeout(callback: () => void | Promise<void>) {
      const id = nextId;
      nextId += 1;
      pending.set(id, { id, callback });
      return id;
    },
    clearTimeout(id: number | ReturnType<typeof setTimeout>) {
      assert.equal(typeof id, "number");
      pending.delete(Number(id));
    },
    pendingCount() {
      return pending.size;
    },
    async runNext() {
      const timer = pending.values().next().value;
      assert.ok(timer, "expected pending timer");
      pending.delete(timer.id);
      await timer.callback();
    },
  };
}
