import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ProphTile } from "@prophetica/ui";
import { ProbabilityBar } from "@/components/probability-bar";
import { ProbabilityChart } from "@/components/probability-chart";
import { TradePanel } from "@/components/trade-panel";
import {
  categoryLabel,
  formatCloseAt,
  formatPoints,
  getChartSeries,
  getRecentTrades,
  type Market,
  type RecentTrade,
  statusLabel,
} from "@/lib/mock-markets";
import { fetchOrderBookSnapshot, type EngineOrderBookSnapshot } from "@/lib/engine-api";
import { fetchMarketById } from "@/lib/market-api";

interface PageProps {
  // Next.js 15: params is a Promise.
  params: Promise<{ id: string }>;
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { id } = await params;
  const { market } = await fetchMarketById(id);
  if (!market) return { title: "找不到市場 — prophetica" };
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

  return (
    <>
      <ProphTile
        surface="light"
        kicker={`${categoryLabel[market.category]} · ${statusLabel[market.status]}`}
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
                價格為 YES 一股的最後成交價（單位：cent，1 cent = 0.01 點）。買入 YES 一股表示「我認為會發生」，
                結算為 YES 時每股入帳 100 點。
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

function ChartSection({ market }: { market: Market }) {
  const data = getChartSeries(market);
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
          <header className="mb-4 flex items-baseline justify-between">
            <div>
              <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">YES 價格走勢</h2>
              <p className="mt-1 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                過去 30 天，每日收盤價（單位：cent）。資料目前為示意；後端時間序列就緒後即時更新。
              </p>
            </div>
          </header>
          <ProbabilityChart data={data} />
        </article>
      </div>
    </section>
  );
}

function ResolvedBanner({ market }: { market: Market }) {
  const isYes = market.resolvedOutcome === "YES";
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
      <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
        最終結果
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
        於 {formatCloseAt(market.resolvedAt ?? market.closeAt)} 由 Oracle 判定並進入結算流水線。
        持倉已自動兌換為點數，可於個人持倉頁查看。
      </p>
    </article>
  );
}

async function OpenMarketBody({ market }: { market: Market }) {
  const book = await fetchOrderBookSnapshot({ marketId: market.id });
  const trades = getRecentTrades(market);

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 pb-24 md:px-10 lg:pb-16">
        <div className="grid grid-cols-1 gap-8 lg:grid-cols-[1fr_360px]">
          <div className="space-y-8">
            <OrderBookTable book={book} />
            <RecentTradesTable trades={trades} />
          </div>
          <TradePanel market={market} />
        </div>
      </div>
    </section>
  );
}

function OrderBookTable({ book }: { book: EngineOrderBookSnapshot }) {
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas">
      <header className="border-b border-proph-divider-soft px-6 py-4">
        <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">YES 訂單簿</h2>
        <p className="mt-1 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
          顯示 YES 一邊的買賣盤；NO 訂單簿為其鏡像（NO 價 = 100 − YES 價）。
        </p>
      </header>

      <div className="grid grid-cols-2 divide-x divide-proph-divider-soft">
        <div className="p-6">
          <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            買盤（Bids）
          </h3>
          <table className="mt-3 w-full font-mono text-[14px] tabular-nums">
            <thead>
              <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
                <th className="pb-2">價格</th>
                <th className="pb-2 text-right">數量</th>
              </tr>
            </thead>
            <tbody>
              {book.bids.map((row) => (
                <tr key={`bid-${row.priceCents}`} className="border-b border-proph-divider-soft last:border-0">
                  <td className="py-2 text-proph-primary">{row.priceCents}¢</td>
                  <td className="py-2 text-right text-proph-ink-muted-80">{row.qty.toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="p-6">
          <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            賣盤（Asks）
          </h3>
          <table className="mt-3 w-full font-mono text-[14px] tabular-nums">
            <thead>
              <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
                <th className="pb-2">價格</th>
                <th className="pb-2 text-right">數量</th>
              </tr>
            </thead>
            <tbody>
              {book.asks.map((row) => (
                <tr key={`ask-${row.priceCents}`} className="border-b border-proph-divider-soft last:border-0">
                  <td className="py-2 text-proph-ink">{row.priceCents}¢</td>
                  <td className="py-2 text-right text-proph-ink-muted-80">{row.qty.toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </article>
  );
}

function RecentTradesTable({ trades }: { trades: RecentTrade[] }) {
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas">
      <header className="border-b border-proph-divider-soft px-6 py-4">
        <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">近期成交</h2>
      </header>
      <table className="w-full font-mono text-[14px] tabular-nums">
        <thead>
          <tr className="border-b border-proph-divider-soft text-left text-[12px] text-proph-ink-muted-48">
            <th className="px-6 py-3">時間</th>
            <th className="px-6 py-3">方向</th>
            <th className="px-6 py-3 text-right">價格</th>
            <th className="px-6 py-3 text-right">數量</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((t, i) => (
            <tr key={i} className="border-b border-proph-divider-soft last:border-0">
              <td className="px-6 py-2 text-proph-ink-muted-48">{t.ago}</td>
              <td className={"px-6 py-2 " + (t.side === "YES" ? "text-proph-primary" : "text-proph-ink")}>
                {t.side}
              </td>
              <td className="px-6 py-2 text-right text-proph-ink">{t.priceCents}¢</td>
              <td className="px-6 py-2 text-right text-proph-ink-muted-80">{t.qty}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </article>
  );
}

function MarketInfoBlock({ market }: { market: Market }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">市場資訊</h2>
        <dl className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
          <Field label="Oracle 來源" value={market.oracleSource} />
          <Field label="開盤" value={formatCloseAt(market.openedAt)} />
          <Field label="關盤" value={formatCloseAt(market.closeAt)} />
          <Field label="累計成交量" value={formatPoints(market.volumeTotalPoints)} />
        </dl>

        <p className="mt-8 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          市場合規邊界遵循 SPEC-015 §5；Oracle 結算流程與申訴窗口遵循 SPEC-004。
          如對結算結果有疑問，請於結算後 24 小時內透過{" "}
          <a href="/support/disputes" className="text-proph-primary underline">
            申訴流程
          </a>
          提起。
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
