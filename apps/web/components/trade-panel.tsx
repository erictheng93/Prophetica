"use client";

import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useTranslations } from "next-intl";
import { useRouter } from "next/navigation";
import { TrendDown, TrendUp } from "@phosphor-icons/react";
import { orderRejectionRedirectPath, placeMarketOrder } from "@/lib/engine-api";
import type {
  PlaceMarketOrderResult,
  QueuedMarketOrder,
} from "@/lib/engine-api";
import type { JourneyState } from "@/lib/journey-state";
import type { Market } from "@/lib/mock-markets";
import { createOfflineMutationQueue } from "@/lib/offline-mutation-queue";

/*
 * Order entry form per SPEC-003 §3 (cent-priced YES/NO outcomes) + ADR-0004
 * §9 #6 (forms must use React Hook Form + Zod once they actually submit).
 *
 * Responsive behavior:
 *   - Desktop (≥ lg): sticky right column.
 *   - Mobile (< lg):  floating bottom summary bar; tapping opens a fullscreen
 *                     bottom sheet. Same form instance, CSS-positioned only.
 */

export function TradePanel({
  market,
  journeyState,
}: {
  market: Market;
  journeyState?: JourneyState;
}) {
  const tValidation = useTranslations("tradePanel.validation");
  const tFlash = useTranslations("tradePanel");
  const tSummary = useTranslations("tradePanel.summary");
  const tPanel = useTranslations("tradePanel.panel");
  const router = useRouter();
  const [hydrated, setHydrated] = useState(false);

  const tradeSchema = z.object({
    side: z.enum(["YES", "NO"]),
    priceCents: z.coerce
      .number()
      .int(tValidation("priceInteger"))
      .min(1, tValidation("priceMin"))
      .max(99, tValidation("priceMax")),
    quantity: z.coerce
      .number()
      .int(tValidation("qtyInteger"))
      .positive(tValidation("qtyPositive"))
      .max(100_000, tValidation("qtyMax")),
  });
  type TradeInput = z.input<typeof tradeSchema>;
  type TradeOutput = z.output<typeof tradeSchema>;

  const [sheetOpen, setSheetOpen] = useState(false);
  const [flash, setFlash] = useState<{
    kind: "success" | "error";
    message: string;
  } | null>(null);
  const [pendingMutation, setPendingMutation] = useState<{
    kind: "queued" | "replayed" | "failed";
    idempotencyKey?: string;
  } | null>(null);

  const isTradable = market.status === "open" || market.status === "closing";

  useEffect(() => {
    setHydrated(true);
  }, []);

  const drainPendingMutations = useCallback(async () => {
    if (typeof window === "undefined") return;
    try {
      const result = await createOfflineMutationQueue().drain();
      navigator.serviceWorker?.controller?.postMessage({
        type: "DRAIN_OFFLINE_MUTATIONS",
      });
      if (result.replayed > 0) {
        setPendingMutation({ kind: "replayed" });
      } else if (result.failed > 0) {
        setPendingMutation({ kind: "failed" });
      }
    } catch {
      setPendingMutation((current) =>
        current?.kind === "queued" ? { ...current, kind: "failed" } : current,
      );
    }
  }, []);

  useEffect(() => {
    function onOnline() {
      void drainPendingMutations();
    }
    function onVisibilityChange() {
      if (document.visibilityState === "visible") {
        void drainPendingMutations();
      }
    }

    window.addEventListener("online", onOnline);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.removeEventListener("online", onOnline);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [drainPendingMutations]);

  const {
    register,
    handleSubmit,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<TradeInput, undefined, TradeOutput>({
    resolver: zodResolver(tradeSchema),
    defaultValues: {
      side: "YES",
      priceCents: market.yesAskCents,
      quantity: 10,
    },
    mode: "onChange",
  });

  const side = watch("side");
  const priceCents = Number(watch("priceCents"));
  const quantity = Number(watch("quantity"));
  const maxCost =
    Number.isFinite(priceCents) &&
    Number.isFinite(quantity) &&
    priceCents > 0 &&
    quantity > 0
      ? (priceCents * quantity) / 100
      : 0;

  const sideLabel = side === "YES" ? "YES" : "NO";
  const sideQuote =
    side === "YES" ? market.yesAskCents : 100 - market.yesBidCents;

  const onSubmit = async (values: TradeOutput) => {
    const returnTo = currentReturnToPath(market.id);
    const softGateRedirect = orderSoftGateRedirectPath(
      journeyState,
      values,
      returnTo,
    );
    if (softGateRedirect) {
      router.push(softGateRedirect);
      return;
    }

    try {
      // No `userId` / `bearerToken` here: this hits the same-origin BFF
      // proxy at /api/engine/markets/:id/orders, which derives the caller
      // identity server-side from the proph_session cookie (see
      // apps/web/app/api/engine/markets/[marketId]/orders/route.ts).
      // Forwarding a browser-supplied `x-user-id` would be misleading —
      // the BFF discards it before calling api-gateway.
      const order = await placeMarketOrder({
        marketId: market.id,
        side: "buy",
        outcome: values.side,
        type: "LIMIT",
        pricePts: values.priceCents,
        quantity: values.quantity,
        timeInForce: "GTC",
      });
      if (isQueuedMarketOrder(order)) {
        setPendingMutation({
          kind: "queued",
          idempotencyKey: order.idempotencyKey,
        });
        setFlash(null);
        setSheetOpen(false);
        return;
      }
      setFlash({
        kind: "success",
        message: tFlash("flashAccepted", {
          side: values.side,
          quantity: values.quantity.toLocaleString(),
          price: values.priceCents,
          orderId: order.orderId,
        }),
      });
      setSheetOpen(false);
      setTimeout(() => setFlash(null), 5000);
    } catch (error) {
      const redirectPath = orderRejectionRedirectPath(error, returnTo);
      if (redirectPath) {
        router.push(redirectPath);
        return;
      }

      setFlash({
        kind: "error",
        message: tFlash("flashRejected", {
          message:
            error instanceof Error ? error.message : tFlash("unknownError"),
        }),
      });
    }
  };

  if (!isTradable) return null;

  return (
    <>
      {/* Mobile floating summary bar — hidden when the sheet is open */}
      <button
        type="button"
        onClick={() => setSheetOpen(true)}
        className={
          "fixed inset-x-0 bottom-0 z-30 flex items-center justify-between border-t border-proph-hairline bg-proph-canvas/95 px-5 py-3 backdrop-blur lg:hidden " +
          (sheetOpen ? "hidden" : "")
        }
        aria-label={tSummary("openLabel")}
      >
        <span className="flex flex-col items-start">
          <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
            {tSummary("stockUnitTemplate", {
              side: sideLabel,
              price: priceCents || sideQuote,
              quantity: quantity || 0,
            })}
          </span>
          <span className="mt-1 font-mono text-[17px] font-semibold tabular-nums text-proph-ink">
            {tSummary("maxCostTemplate", {
              amount: maxCost.toLocaleString("zh-Hant"),
            })}
          </span>
        </span>
        <span className="rounded-proph-pill bg-proph-primary px-5 py-2 font-text text-[14px] tracking-[-0.224px] text-proph-on-primary">
          {tSummary("submitCta")}
        </span>
      </button>

      {/* Backdrop on mobile */}
      {sheetOpen ? (
        <button
          type="button"
          onClick={() => setSheetOpen(false)}
          aria-label={tPanel("backdropLabel")}
          className="fixed inset-0 z-40 bg-proph-ink/40 lg:hidden"
        />
      ) : null}

      {/*
       * Single panel — CSS positions it differently per breakpoint:
       *   - Mobile default: hidden below the viewport.
       *   - Mobile open:    fullscreen bottom sheet with rounded top.
       *   - Desktop:        relative, sticky aside in the right column.
       */}
      <aside
        aria-label={tPanel("ariaLabel")}
        data-hydrated={hydrated ? "true" : "false"}
        data-journey-status={journeyState?.status ?? "unknown"}
        className={
          "fixed inset-x-0 bottom-0 z-50 max-h-[88vh] overflow-y-auto rounded-t-proph-lg border-t border-proph-hairline bg-proph-canvas p-6 transition-transform duration-300 " +
          (sheetOpen ? "translate-y-0" : "translate-y-full") +
          " lg:static lg:inset-auto lg:z-auto lg:max-h-none lg:translate-y-0 lg:rounded-proph-xl lg:border lg:border-proph-hairline/80 lg:shadow-proph-card lg:overflow-visible lg:sticky lg:top-[140px] lg:self-start"
        }
      >
        <div className="flex items-baseline justify-between">
          <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
            {tPanel("heading")}
          </h2>
          <button
            type="button"
            onClick={() => setSheetOpen(false)}
            aria-label={tPanel("closeAriaLabel")}
            className="font-text text-[14px] text-proph-ink-muted-48 hover:text-proph-ink lg:hidden"
          >
            {tPanel("closeLabel")}
          </button>
        </div>
        <p className="mt-1 font-text text-[12px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          {tPanel("complianceNote")}
        </p>

        <form
          onSubmit={handleSubmit(onSubmit)}
          className="mt-5 space-y-5"
          noValidate
        >
          {/* Side selector — sr-only radios + has-[:checked] visual */}
          <fieldset>
            <legend className="sr-only">{tPanel("sideLegend")}</legend>
            <div className="grid grid-cols-2 gap-2">
              <label className="cursor-pointer rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary has-[:checked]:bg-proph-canvas-parchment">
                <input
                  type="radio"
                  {...register("side")}
                  value="YES"
                  defaultChecked
                  className="sr-only"
                />
                <span className="block font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  BUY
                </span>
                <span className="mt-1 inline-flex items-center gap-1 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-primary">
                  <TrendUp
                    weight="bold"
                    className="h-6 w-6 shrink-0"
                    aria-hidden
                  />
                  YES
                </span>
                <span className="mt-1 block font-mono text-[14px] tabular-nums text-proph-ink-muted-80">
                  {market.yesAskCents}¢
                </span>
              </label>
              <label className="cursor-pointer rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary has-[:checked]:bg-proph-canvas-parchment">
                <input
                  type="radio"
                  {...register("side")}
                  value="NO"
                  className="sr-only"
                />
                <span className="block font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  BUY
                </span>
                <span className="mt-1 inline-flex items-center gap-1 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-ink">
                  <TrendDown
                    weight="bold"
                    className="h-6 w-6 shrink-0"
                    aria-hidden
                  />
                  NO
                </span>
                <span className="mt-1 block font-mono text-[14px] tabular-nums text-proph-ink-muted-80">
                  {100 - market.yesBidCents}¢
                </span>
              </label>
            </div>
          </fieldset>

          <FormField
            label={tPanel("priceLabel")}
            hint={tPanel("priceHint", { side: sideLabel, quote: sideQuote })}
            error={errors.priceCents?.message}
          >
            <input
              type="number"
              inputMode="numeric"
              step={1}
              min={1}
              max={99}
              {...register("priceCents")}
              className="w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none aria-[invalid=true]:border-proph-feedback-error"
              aria-invalid={!!errors.priceCents}
            />
          </FormField>

          <FormField
            label={tPanel("qtyLabel")}
            hint={tPanel("qtyHint")}
            error={errors.quantity?.message}
          >
            <input
              type="number"
              inputMode="numeric"
              step={1}
              min={1}
              {...register("quantity")}
              className="w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none aria-[invalid=true]:border-proph-feedback-error"
              aria-invalid={!!errors.quantity}
            />
          </FormField>

          <div className="flex items-baseline justify-between border-t border-proph-divider-soft pt-4">
            <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              {tPanel("maxCostLabel")}
            </span>
            <span className="font-mono text-[21px] font-semibold tabular-nums text-proph-ink">
              {tPanel("pointsTemplate", {
                amount: maxCost.toLocaleString("zh-Hant"),
              })}
            </span>
          </div>

          {flash ? (
            <p
              role={flash.kind === "error" ? "alert" : "status"}
              className={
                "rounded-proph-md border px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] " +
                (flash.kind === "error"
                  ? "border-proph-feedback-error/50 bg-proph-canvas text-proph-feedback-error"
                  : "border-proph-primary/40 bg-proph-canvas-parchment text-proph-primary")
              }
            >
              {flash.message}
            </p>
          ) : null}

          {pendingMutation ? (
            <p
              role="status"
              className="rounded-proph-md border border-proph-primary/40 bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-primary"
            >
              {pendingMutation.kind === "queued"
                ? tPanel("pendingQueued")
                : pendingMutation.kind === "replayed"
                  ? tPanel("pendingReplayed")
                  : tPanel("pendingFailed")}
            </p>
          ) : null}

          <button
            type="button"
            onClick={handleSubmit(onSubmit)}
            disabled={isSubmitting}
            className="w-full inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary transition-transform duration-150 ease-out active:scale-[0.95] disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
          >
            {isSubmitting ? tPanel("submitting") : tPanel("submit")}
          </button>

          <p className="font-text text-[12px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
            {tPanel("footnote")}
          </p>
        </form>
      </aside>
    </>
  );
}

function currentReturnToPath(marketId: string): string {
  if (typeof window === "undefined") {
    return `/markets/${encodeURIComponent(marketId)}`;
  }
  return `${window.location.pathname}${window.location.search}`;
}

function orderSoftGateRedirectPath(
  journeyState: JourneyState | undefined,
  values: { priceCents: number; quantity: number },
  returnTo: string,
): string | null {
  const encodedReturnTo = encodeURIComponent(returnTo);
  if (!journeyState || journeyState.status === "visitor") {
    return `/auth/sign-in?return_to=${encodedReturnTo}`;
  }
  if ((journeyState.kycTier ?? 0) < 1) {
    return `/kyc/tier-1?return_to=${encodedReturnTo}`;
  }

  const requiredMinorUnits = Math.ceil(
    values.priceCents * values.quantity * 1.02,
  );
  if (
    journeyState.balanceMinor !== undefined &&
    journeyState.balanceMinor < requiredMinorUnits
  ) {
    return `/topup?return_to=${encodedReturnTo}`;
  }

  return null;
}

function isQueuedMarketOrder(
  order: PlaceMarketOrderResult,
): order is QueuedMarketOrder {
  return "kind" in order && order.kind === "queued";
}

interface FormFieldProps {
  label: string;
  hint?: string;
  error?: string;
  children: React.ReactNode;
}

function FormField({ label, hint, error, children }: FormFieldProps) {
  return (
    <label className="block">
      <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
        {label}
      </span>
      <div className="mt-1">{children}</div>
      {error ? (
        <span
          role="alert"
          className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-feedback-error"
        >
          {error}
        </span>
      ) : hint ? (
        <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
          {hint}
        </span>
      ) : null}
    </label>
  );
}
