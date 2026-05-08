"use client";

import { useEffect, useRef, type Dispatch } from "react";

import type { KycOverview } from "@/lib/kyc-api";
import type { KycPhase, KycPhaseAction } from "@/lib/kyc-state-machine";
import { createKycWebhookPoller } from "@/lib/kyc-webhook-polling";

export function useKycWebhookPolling(
  phase: KycPhase,
  dispatch: Dispatch<KycPhaseAction>,
  refreshKycOverview: () => Promise<KycOverview>,
) {
  const pollerRef = useRef<ReturnType<typeof createKycWebhookPoller> | null>(null);

  useEffect(() => {
    return () => {
      pollerRef.current?.stop();
      pollerRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (phase.kind !== "awaiting_webhook") {
      return;
    }
    pollerRef.current = createKycWebhookPoller({
      submittedAtMs: phase.submittedAtMs,
      refreshKycOverview,
      dispatch,
    });
    return () => {
      pollerRef.current?.stop();
      pollerRef.current = null;
    };
  }, [dispatch, phase, refreshKycOverview]);
}
