import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KYC_WEBHOOK_POLL_INTERVAL_MS,
  KYC_WEBHOOK_POLL_TIMEOUT_MS,
  createKycWebhookPoller,
} from "../lib/kyc-webhook-polling.js";
import type { KycPhaseAction } from "../lib/kyc-state-machine.js";
import type { KycStatus } from "@prophetica/ts-sdk";

// Minimal manual fake-timer harness so we can drive the polling lifecycle
// deterministically without bringing in @sinon/fake-timers. The poller
// accepts injectable `now` / `setTimeout` / `clearTimeout` exactly so tests
// can do this without monkey-patching globals.

interface ScheduledTask {
  id: number;
  fireAtMs: number;
  callback: () => void;
}

// Match the contravariant TimerHandle = ReturnType<typeof setTimeout> | number
// from kyc-webhook-polling.ts so the poller accepts our fake without casts.
type FakeTimerHandle = ReturnType<typeof setTimeout> | number;

function createFakeClock(initialMs = 0) {
  let now = initialMs;
  let nextId = 1;
  const tasks: ScheduledTask[] = [];

  return {
    now: () => now,
    setTimeout(callback: () => void, delayMs: number): FakeTimerHandle {
      const task: ScheduledTask = {
        id: nextId++,
        fireAtMs: now + delayMs,
        callback,
      };
      tasks.push(task);
      return task.id;
    },
    clearTimeout(handle: FakeTimerHandle) {
      const id = typeof handle === "number" ? handle : Number(handle);
      const idx = tasks.findIndex((t) => t.id === id);
      if (idx !== -1) tasks.splice(idx, 1);
    },
    /** Advance by `deltaMs` and synchronously fire any tasks whose fire time
     * is now in the past. Returns the number of tasks fired. */
    async advance(deltaMs: number): Promise<number> {
      now += deltaMs;
      let fired = 0;
      while (true) {
        const due = tasks
          .filter((t) => t.fireAtMs <= now)
          .sort((a, b) => a.fireAtMs - b.fireAtMs);
        if (due.length === 0) break;
        const task = due[0]!;
        tasks.splice(tasks.indexOf(task), 1);
        task.callback();
        fired++;
        // Allow microtasks (the async tick) to drain between timer callbacks.
        await Promise.resolve();
        await Promise.resolve();
      }
      return fired;
    },
    pendingCount: () => tasks.length,
  };
}

test("poller dispatches polling_timeout once the 5-minute budget is exceeded", async () => {
  const clock = createFakeClock(0);
  const dispatched: KycPhaseAction[] = [];

  const poller = createKycWebhookPoller({
    submittedAtMs: 0,
    refreshKycOverview: async () => ({ status: "pending" as KycStatus }),
    dispatch: (a) => dispatched.push(a),
    now: clock.now,
    setTimeout: clock.setTimeout,
    clearTimeout: clock.clearTimeout,
  });

  // Advance past the 5-minute cap; budget is exceeded on the first tick after.
  await clock.advance(KYC_WEBHOOK_POLL_TIMEOUT_MS + KYC_WEBHOOK_POLL_INTERVAL_MS);

  poller.stop();

  const timeoutAction = dispatched.find((a) => a.type === "polling_timeout");
  assert.ok(timeoutAction, "expected a polling_timeout dispatch");
  // No status_passed / status_failed should leak through after timeout.
  assert.equal(
    dispatched.filter((a) => a.type !== "polling_timeout").length,
    0,
  );
});

test("poller emits status_passed and stops once backend reports passed", async () => {
  const clock = createFakeClock(1_000);
  const dispatched: KycPhaseAction[] = [];
  let callCount = 0;

  const poller = createKycWebhookPoller({
    submittedAtMs: 1_000,
    refreshKycOverview: async () => {
      callCount++;
      // First poll still pending, second poll passes.
      return { status: callCount === 1 ? "pending" : "passed" } as {
        status: KycStatus;
      };
    },
    dispatch: (a) => dispatched.push(a),
    now: clock.now,
    setTimeout: clock.setTimeout,
    clearTimeout: clock.clearTimeout,
  });

  await clock.advance(KYC_WEBHOOK_POLL_INTERVAL_MS); // first tick: pending
  await clock.advance(KYC_WEBHOOK_POLL_INTERVAL_MS); // second tick: passed

  poller.stop();

  assert.equal(callCount, 2, "expected exactly two polls before completion");
  assert.deepEqual(
    dispatched.filter((a) => a.type === "status_passed"),
    [{ type: "status_passed" }],
  );
  // Once status_passed fires, no further timers should be scheduled.
  assert.equal(clock.pendingCount(), 0);
});

test("poller treats refresh failure as transient and keeps polling", async () => {
  const clock = createFakeClock(0);
  const dispatched: KycPhaseAction[] = [];
  let attempts = 0;

  const poller = createKycWebhookPoller({
    submittedAtMs: 0,
    refreshKycOverview: async () => {
      attempts++;
      if (attempts < 3) throw new Error("transient network failure");
      return { status: "passed" as KycStatus };
    },
    dispatch: (a) => dispatched.push(a),
    now: clock.now,
    setTimeout: clock.setTimeout,
    clearTimeout: clock.clearTimeout,
  });

  await clock.advance(KYC_WEBHOOK_POLL_INTERVAL_MS);
  await clock.advance(KYC_WEBHOOK_POLL_INTERVAL_MS);
  await clock.advance(KYC_WEBHOOK_POLL_INTERVAL_MS);

  poller.stop();

  assert.equal(attempts, 3);
  assert.deepEqual(
    dispatched,
    [{ type: "status_passed" }],
    "transient errors must not surface as user-visible failures",
  );
});

test("poller.stop() cancels outstanding timers and prevents further dispatch", async () => {
  const clock = createFakeClock(0);
  const dispatched: KycPhaseAction[] = [];

  const poller = createKycWebhookPoller({
    submittedAtMs: 0,
    refreshKycOverview: async () => ({ status: "pending" as KycStatus }),
    dispatch: (a) => dispatched.push(a),
    now: clock.now,
    setTimeout: clock.setTimeout,
    clearTimeout: clock.clearTimeout,
  });

  poller.stop();
  await clock.advance(KYC_WEBHOOK_POLL_TIMEOUT_MS * 2);

  assert.equal(dispatched.length, 0, "no actions should fire after stop()");
  assert.equal(clock.pendingCount(), 0);
});
