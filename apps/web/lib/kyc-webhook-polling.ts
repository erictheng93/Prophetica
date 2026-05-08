import type { KycStatus } from "@prophetica/ts-sdk";
import type { KycPhaseAction } from "./kyc-state-machine.js";

export const KYC_WEBHOOK_POLL_INTERVAL_MS = 3_000;
export const KYC_WEBHOOK_POLL_TIMEOUT_MS = 5 * 60 * 1_000;

export function pollingActionForKycStatus(
  status: KycStatus,
): Extract<KycPhaseAction, { type: "status_passed" | "status_failed" }> | null {
  if (status === "passed") {
    return { type: "status_passed" };
  }
  if (status === "rejected" || status === "frozen" || status === "expired") {
    return { type: "status_failed", reason: status };
  }
  return null;
}

export interface KycWebhookPoller {
  stop(): void;
}

type TimerHandle = ReturnType<typeof setTimeout> | number;

export interface CreateKycWebhookPollerOptions {
  submittedAtMs: number;
  refreshKycOverview: () => Promise<{ status: KycStatus }>;
  dispatch: (action: KycPhaseAction) => void;
  now?: () => number;
  setTimeout?: (callback: () => void, delayMs: number) => TimerHandle;
  clearTimeout?: (handle: TimerHandle) => void;
}

export function createKycWebhookPoller(
  options: CreateKycWebhookPollerOptions,
): KycWebhookPoller {
  const now = options.now ?? Date.now;
  const scheduleTimeout = options.setTimeout ?? setTimeout;
  const cancelTimeout = options.clearTimeout ?? clearTimeout;
  let stopped = false;
  let timer: TimerHandle | null = null;

  const scheduleNext = () => {
    if (stopped) {
      return;
    }
    timer = scheduleTimeout(() => {
      void tick();
    }, KYC_WEBHOOK_POLL_INTERVAL_MS);
  };

  const tick = async () => {
    timer = null;
    if (stopped) {
      return;
    }
    if (now() - options.submittedAtMs > KYC_WEBHOOK_POLL_TIMEOUT_MS) {
      options.dispatch({ type: "polling_timeout" });
      stopped = true;
      return;
    }

    let overview;
    try {
      overview = await options.refreshKycOverview();
    } catch {
      scheduleNext();
      return;
    }

    if (stopped) {
      return;
    }
    const action = pollingActionForKycStatus(overview.status);
    if (action !== null) {
      options.dispatch(action);
      stopped = true;
      return;
    }

    scheduleNext();
  };

  scheduleNext();

  return {
    stop() {
      stopped = true;
      if (timer !== null) {
        cancelTimeout(timer);
        timer = null;
      }
    },
  };
}
