import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { ProbabilityBar } from "@/components/probability-bar";
import { ProbabilityChart } from "@/components/probability-chart";
import { TradePanel } from "@/components/trade-panel";
import {
  formatCloseAt,
  formatPoints,
  getChartSeries,
  getRecentTrades,
  type Market,
  type RecentTrade,
} from "@/lib/mock-markets";
import {
  fetchOrderBookSnapshot,
  type EngineOrderBookSnapshot,
} from "@/lib/engine-api";
import { getJourneyState } from "@/lib/journey-state";
import { fetchMarketById } from "@/lib/market-api";

interface PageProps {
  // Next.js 15: params is a Promise.
  params: Promise<{ id: string }>;
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { id } = await params;
  const [{ market }, t] = await Promise.all([
    fetchMarketById(id),
    getTranslations("marketDetail"),
  ]);
  if (!market) return { title: t("metadataNotFound") };
  return {
    title: `${market.title} — prophetica`,
    description: market.brief,
  };
}

export default async function MarketDetailPage({ params }: PageProps) {
  const { id } = await params;
  const { market } = await fetchMarketById(id);
  if (!market) notFound();

  const isSettled = market.status === "settled";
  const isOpen = market.status === "open" || market.status === "closing";
  const t = await getTranslations("marketDetail");

  return (
    <>
      <ProphTile
        surface="light"
        headline={market.title}
        tagline={market.brief}
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          {isSettled && market.resolvedOutcome ? (
            <ResolvedBanner market={market} />
          ) : (
            <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
              <ProbabilityBar yesCents={market.yesPriceCents} size="lg" />
              <p className="mt-4 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t("pricingExplainer")}
              </p>
            </article>
          )}
        </div>
      </section>

      <ChartSection market={market} />

      {isOpen ? <OpenMarketBody market={market} /> : null}

      <MarketInfoBlock market={market} />
    </>
  );
}

async function ChartSection({ market }: { market: Market }) {
  const data = getChartSeries(market);
  const t = await getTranslations("marketDetail.chart");
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
          <header className="mb-4 flex items-baseline justify-between">
            <div>
              <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t("title")}
              </h2>
              <p className="mt-1 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                {t("subtitle")}
              </p>
            </div>
          </header>
          <ProbabilityChart data={data} ariaLabel={t("title")} />
        </article>
      </div>
    </section>
  );
}

async function ResolvedBanner({ market }: { market: Market }) {
  const isYes = market.resolvedOutcome === "YES";
  const t = await getTranslations("marketDetail.resolved");
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
      <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
        {t("eyebrow")}
      </div>
      <div
        className={
          "mt-2 font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-none tracking-[-0.28px] " +
          (isYes ? "text-proph-primary" : "text-proph-ink")
        }
      >
        {market.resolvedOutcome}
      </div>
      <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
        {t("body", {
          date: formatCloseAt(market.resolvedAt ?? market.closeAt),
        })}
      </p>
    </article>
  );
}

async function OpenMarketBody({ market }: { market: Market }) {
  const [book, journeyState] = await Promise.all([
    fetchOrderBookSnapshot({ marketId: market.id }),
    getJourneyState(),
  ]);
  const trades = getRecentTrades(market);

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 pb-24 md:px-10 lg:pb-16">
        <div className="grid grid-cols-1 gap-8 lg:grid-cols-[1fr_360px]">
          <div className="space-y-8">
            <OrderBookTable book={book} />
            <RecentTradesTable trades={trades} />
          </div>
          <TradePanel market={market} journeyState={journeyState} />
        </div>
      </div>
    </section>
  );
}

async function OrderBookTable({ book }: { book: EngineOrderBookSnapshot }) {
  const t = await getTranslations("marketDetail.orderBook");
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas">
      <header className="border-b border-proph-divider-soft px-6 py-4">
        <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
          {t("title")}
        </h2>
        <p className="mt-1 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
          {t("subtitle")}
        </p>
      </header>

      <div className="grid grid-cols-2 divide-x divide-proph-divider-soft">
        <div className="p-6">
          <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("bidsHeading")}
          </h3>
          <table className="mt-3 w-full font-mono text-[14px] tabular-nums">
            <thead>
              <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
                <th className="pb-2">{t("priceHeader")}</th>
                <th className="pb-2 text-right">{t("qtyHeader")}</th>
              </tr>
            </thead>
            <tbody>
              {book.bids.map((row) => (
                <tr
                  key={`bid-${row.priceCents}`}
                  className="border-b border-proph-divider-soft last:border-0"
                >
                  <td className="py-2 text-proph-primary">{row.priceCents}¢</td>
                  <td className="py-2 text-right text-proph-ink-muted-80">
                    {row.qty.toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="p-6">
          <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("asksHeading")}
          </h3>
          <table className="mt-3 w-full font-mono text-[14px] tabular-nums">
            <thead>
              <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
                <th className="pb-2">{t("priceHeader")}</th>
                <th className="pb-2 text-right">{t("qtyHeader")}</th>
              </tr>
            </thead>
            <tbody>
              {book.asks.map((row) => (
                <tr
                  key={`ask-${row.priceCents}`}
                  className="border-b border-proph-divider-soft last:border-0"
                >
                  <td className="py-2 text-proph-ink">{row.priceCents}¢</td>
                  <td className="py-2 text-right text-proph-ink-muted-80">
                    {row.qty.toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </article>
  );
}

async function RecentTradesTable({ trades }: { trades: RecentTrade[] }) {
  const t = await getTranslations("marketDetail.recentTrades");
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas">
      <header className="border-b border-proph-divider-soft px-6 py-4">
        <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
          {t("title")}
        </h2>
      </header>
      <table className="w-full font-mono text-[14px] tabular-nums">
        <thead>
          <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
            <th className="px-6 py-3">{t("timeHeader")}</th>
            <th className="px-6 py-3">{t("sideHeader")}</th>
            <th className="px-6 py-3 text-right">{t("priceHeader")}</th>
            <th className="px-6 py-3 text-right">{t("qtyHeader")}</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((trade, i) => (
            <tr
              key={i}
              className="border-b border-proph-divider-soft last:border-0"
            >
              <td className="px-6 py-2 text-proph-ink-muted-48">{trade.ago}</td>
              <td
                className={
                  "px-6 py-2 " +
                  (trade.side === "YES"
                    ? "text-proph-primary"
                    : "text-proph-ink")
                }
              >
                {trade.side}
              </td>
              <td className="px-6 py-2 text-right text-proph-ink">
                {trade.priceCents}¢
              </td>
              <td className="px-6 py-2 text-right text-proph-ink-muted-80">
                {trade.qty}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </article>
  );
}

async function MarketInfoBlock({ market }: { market: Market }) {
  const t = await getTranslations("marketDetail.info");
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("heading")}
        </h2>
        <dl className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
          <Field label={t("fields.oracleSource")} value={market.oracleSource} />
          <Field
            label={t("fields.openedAt")}
            value={formatCloseAt(market.openedAt)}
          />
          <Field
            label={t("fields.closeAt")}
            value={formatCloseAt(market.closeAt)}
          />
          <Field
            label={t("fields.volumeTotal")}
            value={formatPoints(market.volumeTotalPoints)}
          />
        </dl>

        <p className="mt-8 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          {t.rich("complianceNote", {
            disputes: (chunks) => (
              <a
                href="/support/disputes"
                className="text-proph-primary underline"
              >
                {chunks}
              </a>
            ),
          })}
        </p>
      </div>
    </section>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-proph-md border border-proph-hairline bg-proph-canvas p-4">
      <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {value}
      </dd>
    </div>
  );
}
