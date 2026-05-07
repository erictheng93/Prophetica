"use client";

import { useTranslations } from "next-intl";

/*
 * Connection status banner — single component covering:
 *   - SPEC-008 §7.3 fail-soft (WS max attempts exhausted, polling degraded)
 *   - SPEC-008 §9.2 HALTED state (kill switch engaged)
 *
 * SPEC-008 v0.2.1 explicitly collapses these into one banner so that admin
 * UI / player UX never diverge. Variant is driven by `state`; copy is i18n-
 * routed via next-intl namespace `connectionStatus`.
 *
 * Caller wires `onRetry` → ReconnectingWebSocket.restart() (apps/web/lib/
 * ws-client.ts). The banner does NOT auto-retry — that would violate the
 * §7.3 retry budget (manual user action required after exhaustion).
 */

export type ConnectionStatusVariant = "ws-failed" | "halted" | "offline";

export interface ConnectionStatusBannerProps {
  variant: ConnectionStatusVariant;
  reason?: string;
  estimatedResumeAt?: string;
  onRetry?: () => void;
}

export function ConnectionStatusBanner({
  variant,
  reason,
  estimatedResumeAt,
  onRetry,
}: ConnectionStatusBannerProps) {
  const t = useTranslations("connectionStatus");

  const heading = t(`${variant}.heading`);
  const description = reason ?? t(`${variant}.description`);
  const showRetry = variant !== "halted" && typeof onRetry === "function";

  return (
    <div
      role="alert"
      aria-live="assertive"
      className="sticky top-0 z-40 border-b border-proph-feedback-error/30 bg-proph-feedback-error/10 px-5 py-3"
    >
      <div className="mx-auto flex max-w-screen-xl flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div className="space-y-1">
          <p className="font-display text-[14px] font-semibold tracking-[-0.224px] text-proph-feedback-error">
            {heading}
          </p>
          <p className="font-text text-[12px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            {description}
          </p>
          {estimatedResumeAt ? (
            <p className="font-mono text-[12px] tabular-nums text-proph-ink-muted-80">
              {t("estimatedResumeLabel", { ts: estimatedResumeAt })}
            </p>
          ) : null}
        </div>

        {showRetry ? (
          <button
            type="button"
            onClick={onRetry}
            className="inline-flex shrink-0 select-none items-center justify-center rounded-proph-pill border border-proph-feedback-error bg-proph-canvas px-4 py-2 font-text text-[14px] tracking-[-0.224px] text-proph-feedback-error transition-transform duration-150 active:scale-[0.95]"
          >
            {t("retryCta")}
          </button>
        ) : null}
      </div>
    </div>
  );
}
