"use client";

// SPEC-005a §7.3 — React-side wiring of phase transitions to metric emits.
//
// The hook compares previous phase to current phase and dispatches the
// appropriate KycMetricEvent on transition. All emit values are bounded by
// the KycPhase / KycSessionEvent enums, so no PII or free text can land in
// label cardinality.

import { useEffect, useRef } from "react";
import type { KycProviderName, KycTargetTier } from "@prophetica/ts-sdk";

import {
  emitKycMetric,
  type KycCompletionOutcome,
  type KycMetricEvent,
} from "@/lib/telemetry/kyc-metrics";
import type { KycPhase } from "@/lib/kyc-state-machine";

export interface UseKycMetricsOptions {
  tier: KycTargetTier;
  /** May be null while the launch button has not been pressed yet. */
  providerName: KycProviderName | null;
  /** Test seam — defaults to network emit. */
  emit?: (event: KycMetricEvent) => void;
}

export function useKycMetrics(
  phase: KycPhase,
  options: UseKycMetricsOptions,
): void {
  const previousRef = useRef<KycPhase>(phase);
  const emit = options.emit ?? defaultEmit;

  useEffect(() => {
    const previous = previousRef.current;
    previousRef.current = phase;
    if (previous === phase) return;

    const provider = options.providerName ?? "mock";
    const tier = options.tier;

    // start: idle | terminal -> initializing
    if (
      previous.kind !== "initializing" &&
      previous.kind !== "mounting" &&
      phase.kind === "initializing"
    ) {
      emit({
        name: "proph_kyc_session_started_total",
        labels: { tier, provider },
      });
    }

    // completed: awaiting_webhook -> done | failed
    if (previous.kind === "awaiting_webhook" && phase.kind === "done") {
      emit({
        name: "proph_kyc_session_completed_total",
        labels: { tier, provider, outcome: "passed" },
      });
      emit({
        name: "proph_kyc_webhook_wait_seconds",
        labels: { tier, outcome: "passed" },
        observation: secondsSince(previous.submittedAtMs),
      });
    }
    if (previous.kind === "awaiting_webhook" && phase.kind === "failed") {
      const outcome = phase.reason as KycCompletionOutcome;
      emit({
        name: "proph_kyc_session_completed_total",
        labels: { tier, provider, outcome },
      });
      emit({
        name: "proph_kyc_webhook_wait_seconds",
        labels: { tier, outcome },
        observation: secondsSince(previous.submittedAtMs),
      });
    }

    // abandoned: in_progress / mounting -> abandoned
    if (
      (previous.kind === "in_progress" || previous.kind === "mounting") &&
      phase.kind === "abandoned"
    ) {
      const stepAtAbandon =
        previous.kind === "in_progress" && previous.step
          ? previous.step
          : "unknown";
      emit({
        name: "proph_kyc_session_abandoned_total",
        labels: { tier, provider, step_at_abandon: stepAtAbandon },
      });
    }

    // error: any -> error
    if (phase.kind === "error" && previous.kind !== "error") {
      emit({
        name: "proph_kyc_session_error_total",
        labels: {
          tier,
          provider,
          code: phase.code,
          recoverable: phase.recoverable ? "true" : "false",
        },
      });
    }
  }, [phase, options.tier, options.providerName, emit]);
}

function defaultEmit(event: KycMetricEvent): void {
  // Fire-and-forget: emitKycMetric swallows network errors per its contract.
  void emitKycMetric(event);
}

function secondsSince(startedAtMs: number): number {
  return Math.max(0, (Date.now() - startedAtMs) / 1000);
}
