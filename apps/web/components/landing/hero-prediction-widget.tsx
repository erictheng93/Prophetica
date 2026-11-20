"use client";

import { useState } from "react";
import Link from "next/link";
import {
  AmbientGlow,
  NumberTicker,
  ProphBadge,
  TiltCard,
} from "@prophetica/ui";

export function HeroPredictionWidget() {
  const [selectedSide, setSelectedSide] = useState<"yes" | "no">("yes");
  const [shares, setShares] = useState(100);

  const yesProbability = 68;
  const noProbability = 32;

  const currentPrice = selectedSide === "yes" ? 0.68 : 0.32;
  const potentialReturn = (shares * (1 / currentPrice)).toFixed(1);
  const potentialProfitPercent = Math.round((1 / currentPrice - 1) * 100);

  return (
    <div className="relative mx-auto w-full max-w-2xl px-2 sm:px-4">
      <AmbientGlow
        variant="brand"
        size="lg"
        className="left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2"
      />

      <TiltCard
        maxTilt={5}
        spotlightColor="rgba(0, 102, 204, 0.12)"
        className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas/90 p-6 shadow-proph-card backdrop-blur-xl sm:p-8"
      >
        {/* Top telemetry bar */}
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-proph-divider-soft pb-4">
          <div className="flex items-center gap-2">
            <ProphBadge tone="brand" pulse size="sm">
              即時撮合中
            </ProphBadge>
            <span className="font-text text-[12px] text-proph-ink-muted-80">
              世界盃決賽 · 2026-07-19
            </span>
          </div>
          <div className="flex items-center gap-1.5 font-mono text-[12px] text-proph-ink-muted-80">
            <span>24h 量:</span>
            <NumberTicker
              value={1428500}
              prefix="$"
              suffix=" TWD"
              className="font-semibold text-proph-ink"
            />
          </div>
        </div>

        {/* Main Event Question */}
        <div className="my-5">
          <h3 className="font-display text-[20px] font-semibold leading-tight text-proph-ink sm:text-[24px]">
            阿根廷隊是否能衛冕 2026 國際足總世界盃冠軍？
          </h3>
          <p className="mt-1.5 font-text text-[13px] text-proph-ink-muted-80">
            中立撮合訂單簿 · 零對賭風險 · 即時公開清算
          </p>
        </div>

        {/* Probability Dual Bar */}
        <div className="my-4">
          <div className="flex justify-between font-mono text-[14px] font-semibold">
            <span className="flex items-center gap-1.5 text-blue-600 dark:text-blue-400">
              <span className="h-2 w-2 rounded-full bg-blue-500" />
              YES {yesProbability}%
            </span>
            <span className="flex items-center gap-1.5 text-rose-600 dark:text-rose-400">
              NO {noProbability}%
              <span className="h-2 w-2 rounded-full bg-rose-500" />
            </span>
          </div>

          <div className="mt-2 flex h-3 w-full overflow-hidden rounded-full bg-zinc-100 p-0.5 dark:bg-zinc-800">
            <div
              className="h-full rounded-l-full bg-gradient-to-r from-blue-500 to-sky-400 transition-all duration-500"
              style={{ width: `${yesProbability}%` }}
            />
            <div
              className="h-full rounded-r-full bg-gradient-to-r from-rose-400 to-rose-500 transition-all duration-500"
              style={{ width: `${noProbability}%` }}
            />
          </div>
        </div>

        {/* Interactive Side Selector */}
        <div className="my-5 grid grid-cols-2 gap-3">
          <button
            type="button"
            onClick={() => setSelectedSide("yes")}
            className={
              "flex flex-col items-center justify-center rounded-proph-md border p-3.5 transition-all " +
              (selectedSide === "yes"
                ? "border-blue-500 bg-blue-500/10 text-blue-600 shadow-sm dark:bg-blue-500/20 dark:text-blue-400"
                : "border-proph-hairline bg-proph-canvas-parchment text-proph-ink-muted-80 hover:border-proph-primary/40")
            }
          >
            <span className="font-display text-[16px] font-bold">預測 YES</span>
            <span className="mt-0.5 font-mono text-[13px]">$0.68 / 份</span>
          </button>

          <button
            type="button"
            onClick={() => setSelectedSide("no")}
            className={
              "flex flex-col items-center justify-center rounded-proph-md border p-3.5 transition-all " +
              (selectedSide === "no"
                ? "border-rose-500 bg-rose-500/10 text-rose-600 shadow-sm dark:bg-rose-500/20 dark:text-rose-400"
                : "border-proph-hairline bg-proph-canvas-parchment text-proph-ink-muted-80 hover:border-proph-primary/40")
            }
          >
            <span className="font-display text-[16px] font-bold">預測 NO</span>
            <span className="mt-0.5 font-mono text-[13px]">$0.32 / 份</span>
          </button>
        </div>

        {/* Interactive Shares Quick Selector */}
        <div className="flex items-center justify-between rounded-proph-md bg-proph-canvas-parchment px-4 py-2.5">
          <span className="font-text text-[13px] text-proph-ink-muted-80">
            模擬投入份數
          </span>
          <div className="flex items-center gap-1.5">
            {[50, 100, 500].map((amt) => (
              <button
                key={amt}
                type="button"
                onClick={() => setShares(amt)}
                className={
                  "rounded-proph-pill px-2.5 py-1 font-mono text-[12px] transition-colors " +
                  (shares === amt
                    ? "bg-proph-primary text-white"
                    : "bg-proph-canvas text-proph-ink hover:bg-proph-divider-soft")
                }
              >
                {amt} 份
              </button>
            ))}
          </div>
        </div>

        {/* Projected Outcome Payout Box */}
        <div className="mt-4 flex items-center justify-between border-t border-proph-divider-soft pt-4">
          <div>
            <span className="block font-text text-[11px] uppercase tracking-wider text-proph-ink-muted-80">
              預估結算價值
            </span>
            <div className="flex items-baseline gap-1.5">
              <span className="font-mono text-[20px] font-bold text-proph-ink">
                ${potentialReturn}
              </span>
              <span className="font-mono text-[13px] font-medium text-emerald-600 dark:text-emerald-400">
                (+{potentialProfitPercent}%)
              </span>
            </div>
          </div>

          <Link
            href="/markets"
            className="inline-flex items-center gap-1 rounded-proph-pill bg-proph-primary px-4 py-2 font-text text-[14px] font-medium text-proph-on-primary shadow-sm transition-transform hover:brightness-105 active:scale-95"
          >
            探索市場
            <span aria-hidden="true">→</span>
          </Link>
        </div>
      </TiltCard>
    </div>
  );
}
