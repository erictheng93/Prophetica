"use client";

// SPEC-005a §5 ??Tier 2 launch surface. Owns:
//   - Income bucket form (consent + optional declared income)
//   - startKycUpgradeAction call with deterministic idempotency key
//   - selectKycProvider + provider.mount lifecycle
//   - 6-phase state machine (delegated to lib/kyc-state-machine)
// Polling, telemetry, and CSP are deferred to Slices C/D and B respectively.

import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
} from "react";
import { useLocale, useTranslations } from "next-intl";
import {
  selectKycProvider,
  type KycSessionEvent,
  type KycSessionHandle,
} from "@prophetica/ts-sdk";

import {
  refreshKycOverviewAction,
  startKycUpgradeAction,
} from "../_actions";
import { useKycWebhookPolling } from "../use-kyc-webhook-polling";
import { useKycMetrics } from "../use-kyc-metrics";
import { mapLocaleToSumsub } from "@/lib/kyc-locale";
import type { KycProviderName } from "@prophetica/ts-sdk";
import {
  initialKycPhase,
  isRetryablePhase,
  reduceKycPhase,
  type KycPhase,
} from "@/lib/kyc-state-machine";

type IncomeBucketValue = "lt_30k" | "30k_60k" | "60k_100k" | "gt_100k" | "skip";

const incomeBuckets: ReadonlyArray<{
  value: Exclude<IncomeBucketValue, "skip">;
  key: "lt30k" | "30k60k" | "60k100k" | "gt100k";
}> = [
  { value: "lt_30k", key: "lt30k" },
  { value: "30k_60k", key: "30k60k" },
  { value: "60k_100k", key: "60k100k" },
  { value: "gt_100k", key: "gt100k" },
];

export interface Tier2LaunchClientProps {
  /** Tier 1 country (ISO 3166-1 alpha-2). Falls back to "TW" while
   * KycStatusView does not surface country (SPEC-005a §11 open item). */
  country: string;
}

export function Tier2LaunchClient({ country }: Tier2LaunchClientProps) {
  const t = useTranslations("kyc.tier2");
  const tPhase = useTranslations("kyc.tier2.phase");
  const locale = useLocale();

  const [phase, dispatch] = useReducer(reduceKycPhase, initialKycPhase);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [incomeBucket, setIncomeBucket] = useState<IncomeBucketValue>("skip");
  const [providerName, setProviderName] = useState<KycProviderName | null>(null);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const handleRef = useRef<KycSessionHandle | null>(null);

  useKycWebhookPolling(phase, dispatch, refreshKycOverviewAction);
  useKycMetrics(phase, { tier: "TIER_2", providerName });

  useEffect(() => {
    return () => {
      handleRef.current?.unmount();
      handleRef.current = null;
    };
  }, []);

  const launch = useCallback(async () => {
    dispatch({ type: "start" });

    let upgrade: Awaited<ReturnType<typeof startKycUpgradeAction>>;
    try {
      upgrade = await startKycUpgradeAction({
        targetTier: "TIER_2",
        declaredMonthlyIncome: incomeBucketToMinorUnits(incomeBucket),
        retryAttempt,
      });
    } catch {
      dispatch({ type: "upgrade_failed" });
      return;
    }
    dispatch({ type: "upgrade_succeeded" });

    const target = containerRef.current;
    if (!target) {
      dispatch({ type: "mount_failed", code: "unknown" });
      return;
    }

    const provider = selectKycProvider({ sdkToken: upgrade.sdkToken });
    setProviderName(provider.name);
    let handle: KycSessionHandle;
    try {
      handle = await provider.mount(target, {
        sdkToken: upgrade.sdkToken,
        targetTier: "TIER_2",
        // SPEC-005a §2 — SumSub recognises BCP-47 region codes (zh-TW etc),
        // not the script-tagged zh-Hant that next-intl returns.
        locale: mapLocaleToSumsub(locale),
        country,
      });
    } catch {
      // SPEC-005a §2 ??mount() rejects rather than emitting an error event.
      dispatch({ type: "mount_failed", code: "sdk_load_failed" });
      return;
    }
    handleRef.current = handle;
    dispatch({ type: "mount_succeeded" });

    handle.subscribe((event: KycSessionEvent) => {
      dispatch({ type: "sdk_event", event, nowMs: Date.now() });
    });
  }, [country, incomeBucket, locale, retryAttempt]);

  const handleRetry = useCallback(() => {
    handleRef.current?.unmount();
    handleRef.current = null;
    setRetryAttempt((n) => n + 1);
    dispatch({ type: "retry" });
  }, []);

  const phaseLabel = phaseLabelFor(phase, tPhase);
  const showForm = phase.kind === "idle";
  const showStatus = phase.kind !== "idle";
  const showRetry = isRetryablePhase(phase);

  return (
    <>
      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t("sumsub.heading")}
            </h2>
            <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("sumsub.intro")}
            </p>
          </header>

          <div
            ref={containerRef}
            data-testid="kyc-tier2-iframe-host"
            className="flex min-h-[480px] flex-col items-center justify-center gap-3 rounded-proph-lg border-2 border-dashed border-proph-hairline bg-proph-canvas-parchment text-center"
            aria-live="polite"
          >
            {showStatus ? (
              <div
                className="flex flex-col items-center gap-2 px-6 py-12"
                data-testid="kyc-tier2-phase"
                data-phase={phase.kind}
              >
                <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t("sumsub.launchKicker")}
                </span>
                <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink-muted-80">
                  {phaseLabel}
                </span>
                {showRetry ? (
                  <button
                    type="button"
                    onClick={handleRetry}
                    className="mt-4 inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
                  >
                    {tPhase("retryCta")}
                  </button>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      </section>

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
              {t("income.kicker")}
            </span>
            <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">
              {t("income.heading")}
            </h2>
            <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t.rich("income.intro", {
                strong: (chunks) => (
                  <strong className="font-semibold text-proph-ink">
                    {chunks}
                  </strong>
                ),
              })}
            </p>
          </header>

          <fieldset
            className="space-y-6"
            disabled={!showForm}
            aria-disabled={!showForm}
          >
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <legend className="sr-only">{t("income.legend")}</legend>
              {incomeBuckets.map((b) => (
                <label
                  key={b.value}
                  className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <input
                    type="radio"
                    name="incomeBucket"
                    value={b.value}
                    checked={incomeBucket === b.value}
                    onChange={() => setIncomeBucket(b.value)}
                    className="h-4 w-4 accent-proph-primary"
                  />
                  <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {t(`income.buckets.${b.key}`)}
                  </span>
                </label>
              ))}
              <label className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment md:col-span-2">
                <input
                  type="radio"
                  name="incomeBucket"
                  value="skip"
                  checked={incomeBucket === "skip"}
                  onChange={() => setIncomeBucket("skip")}
                  className="h-4 w-4 accent-proph-primary"
                />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t("income.skipLabel")}
                </span>
                <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                  {t("income.skipHint")}
                </span>
              </label>
            </fieldset>

            <div className="flex justify-end">
              <button
                type="button"
                onClick={launch}
                disabled={!showForm}
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
              >
                {t("income.submit")}
              </button>
            </div>
          </fieldset>
        </div>
      </section>
    </>
  );
}

function incomeBucketToMinorUnits(bucket: IncomeBucketValue): number | null {
  switch (bucket) {
    case "lt_30k":
      return 3_000_000;
    case "30k_60k":
      return 6_000_000;
    case "60k_100k":
      return 10_000_000;
    case "gt_100k":
      return 10_000_001;
    case "skip":
      return null;
  }
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
        // SPEC-005a §6.1 fail-soft: review is in-flight, do not surface as error.
        return tPhase("pollingPending");
      }
      return phase.recoverable
        ? tPhase("errorRecoverable")
        : tPhase("errorTerminal");
  }
}
