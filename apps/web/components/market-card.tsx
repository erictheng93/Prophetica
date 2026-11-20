import { ProphBadge } from "@prophetica/ui";
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
  const isClosing = market.status === "closing" || market.status === "closed";
  const t = await getTranslations("marketCard");

  const badgeTone = isSettled ? "neutral" : isClosing ? "warning" : "brand";

  return (
    <a
      href={`/markets/${market.id}`}
      className="group relative flex flex-col justify-between overflow-hidden rounded-proph-lg border border-proph-hairline/80 bg-proph-canvas p-6 transition-all duration-200 hover:-translate-y-1 hover:border-proph-primary/50 hover:shadow-proph-card"
    >
      <div>
        <div className="flex items-center justify-between gap-3">
          <span className="rounded-proph-pill bg-proph-canvas-parchment px-2.5 py-1 font-text text-[11px] font-semibold uppercase tracking-wider text-proph-ink-muted-80">
            {categoryLabel[market.category]}
          </span>
          <ProphBadge tone={badgeTone} pulse={!isSettled} size="sm">
            {statusLabel[market.status]}
          </ProphBadge>
        </div>

        <h3 className="mt-4 font-display text-[18px] sm:text-[20px] font-semibold leading-[1.2] tracking-tight text-proph-ink transition-colors group-hover:text-proph-primary">
          {market.title}
        </h3>
        <p className="mt-2 font-text text-[13px] leading-[1.45] text-proph-ink-muted-80 line-clamp-2">
          {market.brief}
        </p>
      </div>

      <div className="mt-6">
        <div>
          {isSettled && market.resolvedOutcome ? (
            <SettledBadge
              outcome={market.resolvedOutcome}
              label={t("settledOutcomeLabel")}
            />
          ) : (
            <ProbabilityBar yesCents={market.yesPriceCents} size="compact" />
          )}
        </div>

        <dl className="mt-5 grid grid-cols-2 gap-3 border-t border-proph-divider-soft pt-4 text-proph-ink-muted-80">
          <div>
            <dt className="font-text text-[11px] uppercase tracking-wider text-proph-ink-muted-48">
              {t("volume24hLabel")}
            </dt>
            <dd className="mt-1 font-mono text-[14px] font-semibold text-proph-ink tabular-nums">
              {formatPoints(market.volume24hPoints)}
            </dd>
          </div>
          <div className="text-right">
            <dt className="font-text text-[11px] uppercase tracking-wider text-proph-ink-muted-48">
              {isSettled ? t("settledDateLabel") : t("closeDateLabel")}
            </dt>
            <dd className="mt-1 font-mono text-[13px] text-proph-ink-muted-80 tabular-nums">
              {formatCloseAt(
                isSettled && market.resolvedAt
                  ? market.resolvedAt
                  : market.closeAt,
              )}
            </dd>
          </div>
        </dl>
      </div>
    </a>
  );
}

function SettledBadge({
  outcome,
  label,
}: {
  outcome: "YES" | "NO";
  label: string;
}) {
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
