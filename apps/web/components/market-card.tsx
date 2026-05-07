import { getTranslations } from "next-intl/server";
import {
  categoryLabel,
  formatCloseAt,
  formatPoints,
  type Market,
  statusLabel,
} from "@/lib/mock-markets";
import { ProbabilityBar } from "./probability-bar";

export async function MarketCard({ market }: { market: Market }) {
  const isSettled = market.status === "settled";
  const t = await getTranslations("marketCard");

  return (
    <a
      href={`/markets/${market.id}`}
      className="group block rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {categoryLabel[market.category]}
        </span>
        <span
          className={
            "rounded-proph-pill border px-2 py-0.5 font-text text-[12px] leading-none tracking-[-0.12px] " +
            (isSettled
              ? "border-proph-divider-soft text-proph-ink-muted-48"
              : "border-proph-primary/40 text-proph-primary")
          }
        >
          {statusLabel[market.status]}
        </span>
      </div>

      <h3 className="mt-3 font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink group-hover:underline">
        {market.title}
      </h3>
      <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 line-clamp-2">
        {market.brief}
      </p>

      <div className="mt-5">
        {isSettled && market.resolvedOutcome ? (
          <SettledBadge outcome={market.resolvedOutcome} label={t("settledOutcomeLabel")} />
        ) : (
          <ProbabilityBar yesCents={market.yesPriceCents} size="compact" />
        )}
      </div>

      <dl className="mt-5 grid grid-cols-2 gap-3 border-t border-proph-divider-soft pt-4 text-proph-ink-muted-80">
        <div>
          <dt className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
            {t("volume24hLabel")}
          </dt>
          <dd className="mt-1 font-text text-[14px] tracking-[-0.224px]">
            {formatPoints(market.volume24hPoints)}
          </dd>
        </div>
        <div className="text-right">
          <dt className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
            {isSettled ? t("settledDateLabel") : t("closeDateLabel")}
          </dt>
          <dd className="mt-1 font-text text-[14px] tracking-[-0.224px]">
            {formatCloseAt(isSettled && market.resolvedAt ? market.resolvedAt : market.closeAt)}
          </dd>
        </div>
      </dl>
    </a>
  );
}

function SettledBadge({ outcome, label }: { outcome: "YES" | "NO"; label: string }) {
  const isYes = outcome === "YES";
  return (
    <div className="flex items-center justify-between rounded-proph-md border border-proph-divider-soft bg-proph-canvas-parchment px-4 py-3">
      <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
        {label}
      </span>
      <span
        className={
          "font-display text-[28px] font-semibold leading-none tracking-[-0.374px] " +
          (isYes ? "text-proph-primary" : "text-proph-ink")
        }
      >
        {outcome}
      </span>
    </div>
  );
}
