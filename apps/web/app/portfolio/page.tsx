import type { Metadata } from "next";
import { TrendDown, TrendUp } from "@phosphor-icons/react/dist/ssr";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";
import {
  categoryLabel,
  formatPoints,
  statusLabel,
} from "@/lib/mock-markets";
import {
  formatPointsSigned,
  type PortfolioSummary,
  type ResolvedPosition,
} from "@/lib/mock-portfolio";
import { fetchWalletOverview, type WalletOverview } from "@/lib/api-gateway";
import { fetchPortfolioPositions } from "@/lib/position-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("portfolio.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

type Tone = "neutral" | "positive" | "negative";

function pickTone(value: number): Tone {
  if (value > 0) return "positive";
  if (value < 0) return "negative";
  return "neutral";
}

function toneClass(tone: Tone): string {
  switch (tone) {
    case "positive":
      return "text-proph-feedback-success";
    case "negative":
      return "text-proph-feedback-error";
    default:
      return "text-proph-ink";
  }
}

type PortfolioCopy = Awaited<ReturnType<typeof getTranslations<"portfolio">>>;

export default async function PortfolioPage() {
  const [t, sectionFooterColumns, walletOverview, portfolio] = await Promise.all([
    getTranslations("portfolio"),
    getFooterColumns(["markets", "legal", "support"]),
    fetchWalletOverview(),
    fetchPortfolioPositions(),
  ]);
  const { active, settled, summary } = portfolio;

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
          <p className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <strong className="font-semibold text-proph-ink">{t("demoBanner.label")}</strong>{" "}
            {t("demoBanner.body")}
          </p>
        </div>
      </section>

      <SummaryCards t={t} summary={summary} walletOverview={walletOverview} />

      <LedgerActivitySection t={t} walletOverview={walletOverview} />

      <PositionsSection t={t} positions={active} variant="active" />
      <PositionsSection t={t} positions={settled} variant="settled" />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function SummaryCards({
  t,
  summary,
  walletOverview,
}: {
  t: PortfolioCopy;
  summary: PortfolioSummary;
  walletOverview: WalletOverview;
}) {
  const cards = [
    {
      kicker: t("summary.wallet.kicker"),
      label:
        walletOverview.source === "api-gateway"
          ? t("summary.wallet.labelLive", { userId: walletOverview.balance.userId })
          : t("summary.wallet.labelFallback"),
      value: formatPoints(walletOverview.balance.availablePoints),
      tone: "neutral" as const,
    },
    {
      kicker: t("summary.locked.kicker"),
      label: t("summary.locked.label"),
      value: formatPoints(walletOverview.balance.lockedPoints + walletOverview.balance.frozenPoints),
      tone: "neutral" as const,
    },
    {
      kicker: t("summary.activeCost.kicker"),
      label: t("summary.activeCost.label"),
      value: formatPoints(summary.totalCostPoints),
      tone: "neutral" as const,
    },
    {
      kicker: t("summary.activeValue.kicker"),
      label: t("summary.activeValue.label"),
      value: formatPoints(summary.currentValuePoints),
      tone: "neutral" as const,
    },
    {
      kicker: t("summary.unrealized.kicker"),
      label: t("summary.unrealized.label"),
      value: formatPointsSigned(summary.unrealizedPnlPoints),
      tone: pickTone(summary.unrealizedPnlPoints),
    },
    {
      kicker: t("summary.realized.kicker"),
      label: t("summary.realized.label"),
      value: formatPointsSigned(summary.realizedPnlPointsLifetime),
      tone: pickTone(summary.realizedPnlPointsLifetime),
    },
  ];

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {cards.map((card) => (
            <article
              key={card.kicker}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {card.kicker}
              </div>
              <div
                className={
                  "mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px] " +
                  toneClass(card.tone)
                }
              >
                {card.value}
              </div>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                {card.label}
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function LedgerActivitySection({
  t,
  walletOverview,
}: {
  t: PortfolioCopy;
  walletOverview: WalletOverview;
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-6 flex flex-col gap-2 md:flex-row md:items-end md:justify-between">
          <div>
            <h2 className="font-display text-[34px] font-semibold leading-[1.1]">
              {t("ledger.heading")}
            </h2>
            <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {walletOverview.source === "api-gateway"
                ? t("ledger.subtitleLive")
                : t("ledger.subtitleFallback")}
            </p>
          </div>
          <span className="font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-48">
            {t("ledger.count", { count: walletOverview.ledgerEntries.length })}
          </span>
        </header>

        {walletOverview.ledgerEntries.length === 0 ? (
          <article className="rounded-proph-lg border border-proph-divider-soft bg-proph-canvas-parchment p-6">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("ledger.empty")}
            </p>
          </article>
        ) : (
          <ul className="divide-y divide-proph-divider-soft rounded-proph-lg border border-proph-hairline bg-proph-canvas">
            {walletOverview.ledgerEntries.slice(0, 5).map((entry) => (
              <li
                key={entry.txnId}
                className="grid grid-cols-1 gap-3 p-5 md:grid-cols-[1fr_auto_auto] md:items-center"
              >
                <div className="min-w-0">
                  <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("ledger.txnLabel", { txnId: entry.txnId, direction: entry.direction })}
                  </div>
                  <div className="mt-1 truncate font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                    {entry.accountCode}
                  </div>
                </div>
                <div
                  className={
                    "font-mono text-[17px] tabular-nums tracking-[-0.374px] " +
                    toneClass(pickTone(entry.amountPoints))
                  }
                >
                  {formatPointsSigned(entry.amountPoints)}
                </div>
                <div className="font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-48">
                  {t("ledger.balanceLabel", { balance: formatPoints(entry.balanceAfterPoints) })}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function PositionsSection({
  t,
  positions,
  variant,
}: {
  t: PortfolioCopy;
  positions: ResolvedPosition[];
  variant: "active" | "settled";
}) {
  const scope = variant === "active" ? "positions.active" : "positions.settled";
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8 flex items-baseline justify-between">
          <div>
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t(`${scope}.title`)}
            </h2>
            <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t(`${scope}.subtitle`)}
            </p>
          </div>
          <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("positions.count", { count: positions.length })}
          </span>
        </header>

        {positions.length === 0 ? (
          <article className="rounded-proph-lg border border-proph-divider-soft bg-proph-canvas-parchment p-8 text-center">
            <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t(`${scope}.empty`)}
            </p>
            <ProphLinkButton variant="primary" href="/markets" className="mt-5">
              {t("positions.browseMarkets")}
            </ProphLinkButton>
          </article>
        ) : (
          <ul className="space-y-4">
            {positions.map((p) => (
              <PositionRow key={`${p.marketId}-${p.outcome}`} t={t} position={p} variant={variant} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function PositionRow({
  t,
  position,
  variant,
}: {
  t: PortfolioCopy;
  position: ResolvedPosition;
  variant: "active" | "settled";
}) {
  const { market, outcome, shares, avgCostCents } = position;
  const pnl =
    variant === "active" ? position.unrealizedPnlPoints : (position.realizedPnlPoints ?? 0);
  const pnlTone = pickTone(pnl);
  const valueLabelKey = variant === "active" ? "positions.active.valueLabel" : "positions.settled.valueLabel";
  const pnlLabelKey = variant === "active" ? "positions.active.pnlLabel" : "positions.settled.pnlLabel";

  return (
    <li>
      <a
        href={`/markets/${market.id}`}
        className="block rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
      >
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {categoryLabel[market.category]} · {statusLabel[market.status]}
              </span>
            </div>
            <h3 className="mt-1 font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
              {market.title}
            </h3>
          </div>
          <span
            className={
              "shrink-0 rounded-proph-pill border px-3 py-1 font-display text-[14px] font-semibold tracking-[-0.224px] " +
              (outcome === "YES"
                ? "border-proph-primary text-proph-primary"
                : "border-proph-hairline text-proph-ink")
            }
          >
            {outcome}
          </span>
        </div>

        <dl className="mt-5 grid grid-cols-2 gap-4 border-t border-proph-divider-soft pt-4 md:grid-cols-4">
          <Field label={t("positions.fields.shares")} value={shares.toLocaleString()} mono />
          <Field label={t("positions.fields.avgPrice")} value={`${avgCostCents}¢`} mono />
          <Field
            label={t(valueLabelKey)}
            value={formatPoints(
              variant === "active"
                ? position.currentValuePoints
                : position.totalCostPoints + (position.realizedPnlPoints ?? 0),
            )}
            mono
          />
          <Field
            label={t(pnlLabelKey)}
            value={formatPointsSigned(pnl)}
            mono
            tone={pnlTone}
            extra={
              variant === "active"
                ? `${position.unrealizedPnlPct >= 0 ? "+" : "−"}${Math.abs(position.unrealizedPnlPct).toFixed(1)}%`
                : undefined
            }
          />
        </dl>
      </a>
    </li>
  );
}

function Field({
  label,
  value,
  mono = false,
  tone = "neutral",
  extra,
}: {
  label: string;
  value: string;
  mono?: boolean;
  tone?: Tone;
  extra?: string;
}) {
  return (
    <div>
      <dt className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd
        className={
          "mt-1 inline-flex items-center gap-1 " +
          (mono ? "font-mono tabular-nums text-[17px] tracking-[-0.374px] " : "font-text text-[17px] tracking-[-0.374px] ") +
          toneClass(tone)
        }
      >
        {tone === "positive" ? (
          <TrendUp weight="bold" className="h-4 w-4 shrink-0" aria-hidden="true" />
        ) : tone === "negative" ? (
          <TrendDown weight="bold" className="h-4 w-4 shrink-0" aria-hidden="true" />
        ) : null}
        <span>{value}</span>
        {extra ? (
          <span className="ml-1 font-text text-[12px] tracking-[-0.12px]">{extra}</span>
        ) : null}
      </dd>
    </div>
  );
}
