// SPEC-005a §5.1 — Tier 2/3 KYC page state machine.
//
// Pure reducer: phase + action -> phase. No React, no IO. Side effects
// (fetch, provider.mount, timers) live in the client component which
// dispatches actions back into this reducer.

import type {
  KycErrorCode,
  KycSessionEvent,
  KycStep,
} from "@prophetica/ts-sdk";

export type KycPhase =
  | { kind: "idle" }
  | { kind: "initializing" }
  | { kind: "mounting" }
  | { kind: "in_progress"; step?: KycStep }
  | { kind: "awaiting_stage_complete"; submittedAtMs: number }
  | { kind: "awaiting_webhook"; submittedAtMs: number }
  | { kind: "done" }
  | { kind: "failed"; reason: "rejected" | "frozen" | "expired" }
  | { kind: "abandoned" }
  | {
      kind: "error";
      code: KycErrorCode | "start_upgrade_failed" | "polling_timeout";
      recoverable: boolean;
    };

export type KycPhaseAction =
  | { type: "start" }
  | { type: "upgrade_succeeded" }
  | { type: "upgrade_failed" }
  | { type: "mount_succeeded" }
  | { type: "mount_failed"; code: KycErrorCode }
  | {
      type: "sdk_event";
      event: KycSessionEvent;
      nowMs: number;
      continueAfterSubmit?: boolean;
    }
  | { type: "stage_advance" }
  | { type: "status_passed" }
  | { type: "status_failed"; reason: "rejected" | "frozen" | "expired" }
  | { type: "polling_timeout" }
  | { type: "retry" };

export const initialKycPhase: KycPhase = { kind: "idle" };

export function reduceKycPhase(
  phase: KycPhase,
  action: KycPhaseAction,
): KycPhase {
  switch (action.type) {
    case "start": {
      // Start is only valid from idle / abandoned / error / failed (retry path).
      if (
        phase.kind === "idle" ||
        phase.kind === "abandoned" ||
        phase.kind === "error" ||
        phase.kind === "failed"
      ) {
        return { kind: "initializing" };
      }
      return phase;
    }
    case "upgrade_succeeded": {
      if (phase.kind === "initializing") {
        return { kind: "mounting" };
      }
      return phase;
    }
    case "upgrade_failed": {
      if (phase.kind === "initializing") {
        return {
          kind: "error",
          code: "start_upgrade_failed",
          recoverable: true,
        };
      }
      return phase;
    }
    case "mount_succeeded": {
      // Mount can resolve before or after the SDK's `ready` event drains.
      // We treat both `mounting` and a pre-emptive sdk_event ready as InProgress.
      if (phase.kind === "mounting") {
        return { kind: "in_progress" };
      }
      return phase;
    }
    case "mount_failed": {
      if (phase.kind === "mounting") {
        return {
          kind: "error",
          code: action.code,
          recoverable: isRecoverableErrorCode(action.code),
        };
      }
      return phase;
    }
    case "sdk_event": {
      return reduceSdkEvent(
        phase,
        action.event,
        action.nowMs,
        action.continueAfterSubmit ?? false,
      );
    }
    case "stage_advance": {
      if (phase.kind === "awaiting_stage_complete") {
        return { kind: "mounting" };
      }
      return phase;
    }
    case "status_passed": {
      if (phase.kind === "awaiting_webhook") {
        return { kind: "done" };
      }
      return phase;
    }
    case "status_failed": {
      if (phase.kind === "awaiting_webhook") {
        return { kind: "failed", reason: action.reason };
      }
      return phase;
    }
    case "polling_timeout": {
      if (phase.kind === "awaiting_webhook") {
        return {
          kind: "error",
          code: "polling_timeout",
          recoverable: true,
        };
      }
      return phase;
    }
    case "retry": {
      // Retry from any terminal state collapses back to idle so the user can
      // re-trigger `start`. The retry counter is held by the client component
      // and bumped before the next start.
      if (
        phase.kind === "abandoned" ||
        phase.kind === "error" ||
        phase.kind === "failed"
      ) {
        return { kind: "idle" };
      }
      return phase;
    }
  }
}

function reduceSdkEvent(
  phase: KycPhase,
  event: KycSessionEvent,
  nowMs: number,
  continueAfterSubmit: boolean,
): KycPhase {
  switch (event.kind) {
    case "ready": {
      // The mock provider emits `ready` synchronously after subscribe; the
      // real provider adapter emits via onReady. Both paths converge to in_progress.
      if (phase.kind === "mounting" || phase.kind === "in_progress") {
        return { kind: "in_progress" };
      }
      return phase;
    }
    case "step_changed": {
      if (phase.kind === "in_progress") {
        return { kind: "in_progress", step: event.step };
      }
      return phase;
    }
    case "submitted":
    case "completed": {
      if (phase.kind === "in_progress" || phase.kind === "mounting") {
        if (continueAfterSubmit) {
          return { kind: "awaiting_stage_complete", submittedAtMs: nowMs };
        }
        return { kind: "awaiting_webhook", submittedAtMs: nowMs };
      }
      return phase;
    }
    case "abandoned": {
      if (phase.kind === "in_progress" || phase.kind === "mounting") {
        return { kind: "abandoned" };
      }
      return phase;
    }
    case "error": {
      if (phase.kind === "in_progress" || phase.kind === "mounting") {
        return {
          kind: "error",
          code: event.code,
          recoverable: event.recoverable,
        };
      }
      return phase;
    }
  }
}

function isRecoverableErrorCode(code: KycErrorCode): boolean {
  switch (code) {
    case "sdk_load_failed":
    case "sdk_token_expired":
    case "permission_denied":
    case "network_lost":
      return true;
    case "browser_unsupported":
    case "unknown":
      return false;
  }
}

export function isTerminalPhase(phase: KycPhase): boolean {
  return (
    phase.kind === "done" ||
    phase.kind === "failed" ||
    phase.kind === "abandoned" ||
    phase.kind === "error"
  );
}

export function isRetryablePhase(phase: KycPhase): boolean {
  if (phase.kind === "abandoned") return true;
  if (phase.kind === "error") {
    // Polling timeout is technically recoverable but the back-end review is
    // already in flight (SPEC-005a §6.1: 「審核時間較長，已寄信通知，您可離開頁面」).
    // Retrying would create a new applicant + duplicate webhook, so the UI
    // hides the retry CTA for this specific code.
    if (phase.code === "polling_timeout") return false;
    return phase.recoverable;
  }
  return false;
}

export function isPollingTimeoutPhase(phase: KycPhase): boolean {
  return phase.kind === "error" && phase.code === "polling_timeout";
}
