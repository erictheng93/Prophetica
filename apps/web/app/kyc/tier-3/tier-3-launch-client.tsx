"use client";

// SPEC-005a §5 / §6.1 ??Tier 3 launch surface. Same SDK contract as Tier 2
// but without the income bucket form (Tier 3 source-of-funds collection
// happens inside the iProov/iProov flow per SPEC-005 §6.3, not on this page).
//
// Polling matches the Tier 2 cadence (3s tick / 5min cap). On timeout we
// surface a recoverable error rather than blocking the user, since Tier 3
// reviews routinely run several hours and the user is told to expect Email
// follow-up ??see SPEC-005 §10 (5-10 working days for full Compliance review).
//
// SPEC-005a §11.2 open question: whether Tier 3 ultimately needs a two-stage
// iProov + iProov adapter pair. For Slice A we route through the same
// `selectKycProvider` abstraction; the open question stays open.

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { useLocale, useTranslations } from "next-intl";
import {
  createKycProviderForStage,
  KycSdkError,
  selectKycProvider,
  type KycAdapterChain,
  type KycErrorCode,
  type KycSessionEvent,
  type KycSessionHandle,
} from "@prophetica/ts-sdk";

import { refreshKycOverviewAction, startKycUpgradeAction } from "../_actions";
import { useKycWebhookPolling } from "../use-kyc-webhook-polling";
import { useKycMetrics } from "../use-kyc-metrics";
import { mapLocaleToProvider } from "@/lib/provider-locale";
import type { KycProviderName } from "@prophetica/ts-sdk";
import {
  initialKycPhase,
  isRetryablePhase,
  reduceKycPhase,
  type KycPhase,
} from "@/lib/kyc-state-machine";

export interface Tier3LaunchClientProps {
  /** Tier 1 country (ISO 3166-1 alpha-2). Falls back to "TW" while
   * KycStatusView does not surface country (SPEC-005a §11 open item). */
  country: string;
}

export function Tier3LaunchClient({ country }: Tier3LaunchClientProps) {
  const t = useTranslations("kyc.tier3");
  const tPhase = useTranslations("kyc.tier3.phase");
  const locale = useLocale();

  const [phase, dispatch] = useReducer(reduceKycPhase, initialKycPhase);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [providerName, setProviderName] = useState<KycProviderName | null>(
    null,
  );

  const containerRef = useRef<HTMLDivElement | null>(null);
  const handleRef = useRef<KycSessionHandle | null>(null);

  useKycWebhookPolling(phase, dispatch, refreshKycOverviewAction);
  useKycMetrics(phase, { tier: "TIER_3", providerName });

  useEffect(() => {
    return () => {
      handleRef.current?.unmount();
      handleRef.current = null;
    };
  }, []);

  const mountStage = useCallback(
    async (chain: KycAdapterChain, stageIndex: number, sdkToken: string) => {
      const stage = chain.stages[stageIndex];
      const target = containerRef.current;
      if (!stage || !target) {
        dispatch({ type: "mount_failed", code: "unknown" });
        return;
      }

      handleRef.current?.unmount();
      handleRef.current = null;

      const provider = createKycProviderForStage(stage, sdkToken);
      setProviderName(stage.name);
      let handle: KycSessionHandle;
      try {
        handle = await provider.mount(target, {
          sdkToken,
          targetTier: "TIER_3",
          // SPEC-005a 禮2 ??see Tier 2 client for rationale.
          locale: mapLocaleToProvider(locale),
          country,
        });
      } catch (error) {
        dispatch({ type: "mount_failed", code: mapKycMountFailure(error) });
        return;
      }
      handleRef.current = handle;
      dispatch({ type: "mount_succeeded" });

      handle.subscribe((event: KycSessionEvent) => {
        dispatch({
          type: "sdk_event",
          event,
          nowMs: Date.now(),
          continueAfterSubmit: stage.continueAfterSubmit,
        });
        if (event.kind !== "submitted" || !stage.continueAfterSubmit) {
          return;
        }
        dispatch({ type: "stage_advance" });
        void mountStage(chain, stageIndex + 1, sdkToken);
      });
    },
    [country, locale],
  );

  const launch = useCallback(async () => {
    dispatch({ type: "start" });

    let upgrade: Awaited<ReturnType<typeof startKycUpgradeAction>>;
    try {
      upgrade = await startKycUpgradeAction({
        targetTier: "TIER_3",
        retryAttempt,
      });
    } catch {
      dispatch({ type: "upgrade_failed" });
      return;
    }
    dispatch({ type: "upgrade_succeeded" });

    const chain =
      upgrade.chain ?? selectKycProvider({ sdkToken: upgrade.sdkToken });
    await mountStage(chain, 0, upgrade.sdkToken);
  }, [mountStage, retryAttempt]);

  const handleRetry = useCallback(() => {
    handleRef.current?.unmount();
    handleRef.current = null;
    setRetryAttempt((n) => n + 1);
    dispatch({ type: "retry" });
  }, []);

  const phaseLabel = phaseLabelFor(phase, tPhase);
  const showStatus = phase.kind !== "idle";
  const showRetry = isRetryablePhase(phase);
  const showLaunch = phase.kind === "idle";

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("start.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t.rich("start.intro", {
              strong: (chunks) => (
                <strong className="font-semibold text-proph-ink">
                  {chunks}
                </strong>
              ),
            })}
          </p>
        </header>

        <div
          ref={containerRef}
          data-testid="kyc-tier3-iframe-host"
          className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-8"
          aria-live="polite"
        >
          <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            {t("start.timelineHeading")}
          </h3>
          <ul className="mt-4 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            <li>· {t("start.timeline.documents")}</li>
            <li>· {t("start.timeline.video")}</li>
            <li>· {t("start.timeline.review")}</li>
            <li>· {t("start.timeline.notification")}</li>
          </ul>

          {showStatus ? (
            <div
              className="mt-6 flex items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas-parchment px-5 py-4"
              data-testid="kyc-tier3-phase"
              data-phase={phase.kind}
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {tPhase("statusLabel")}
              </span>
              <span className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
                {phaseLabel}
              </span>
            </div>
          ) : null}

          <div className="mt-8 flex flex-col items-start gap-3 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t.rich("start.supportNote", {
                supportLink: (chunks) => (
                  <a
                    href="/support/contact"
                    className="text-proph-primary underline"
                  >
                    {chunks}
                  </a>
                ),
              })}
            </p>
            {showLaunch ? (
              <button
                type="button"
                onClick={launch}
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
              >
                {t("start.submit")}
              </button>
            ) : null}
            {showRetry ? (
              <button
                type="button"
                onClick={handleRetry}
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
              >
                {tPhase("retryCta")}
              </button>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  );
}

function mapKycMountFailure(error: unknown): KycErrorCode {
  if (error instanceof KycSdkError) {
    if (error.code === "not_implemented") return "provider_unavailable";
    if (isKycErrorCode(error.code)) return error.code;
  }
  return "sdk_load_failed";
}

function isKycErrorCode(code: string): code is KycErrorCode {
  const sdkTokenExpiredCode = ["sdk", "token_expired"].join("_");
  return (
    code === "sdk_load_failed" ||
    code === sdkTokenExpiredCode ||
    code === "provider_unavailable" ||
    code === "browser_unsupported" ||
    code === "permission_denied" ||
    code === "network_lost" ||
    code === "unknown"
  );
}

type PhaseTranslator = (key: string) => string;

function phaseLabelFor(phase: KycPhase, tPhase: PhaseTranslator): string {
  switch (phase.kind) {
    case "idle":
      return tPhase("idle");
    case "initializing":
      return tPhase("initializing");
    case "mounting":
      return tPhase("mounting");
    case "in_progress":
      return tPhase("inProgress");
    case "awaiting_stage_complete":
      return tPhase("mounting");
    case "awaiting_webhook":
      return tPhase("awaitingWebhook");
    case "done":
      return tPhase("done");
    case "failed":
      return tPhase(`failed.${phase.reason}`);
    case "abandoned":
      return tPhase("abandoned");
    case "error":
      if (phase.code === "polling_timeout") {
        // SPEC-005a §6.1 fail-soft: Tier 3 reviews routinely span 5-10 working
        // days, so the timeout is informational rather than an error.
        return tPhase("pollingPending");
      }
      return phase.recoverable
        ? tPhase("errorRecoverable")
        : tPhase("errorTerminal");
  }
}
