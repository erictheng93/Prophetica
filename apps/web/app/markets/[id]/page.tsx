import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { ProbabilityBar } from "@/components/probability-bar";
import {
  categoryLabel,
  formatCloseAt,
  formatPoints,
  getMarketById,
  type Market,
  statusLabel,
} from "@/lib/mock-markets";

interface PageProps {
  // Next.js 15: params is a Promise.
  params: Promise<{ id: string }>;
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { id } = await params;
  const market = getMarketById(id);
  if (!market) return { title: "找不到市場 — prophetica" };
  return {
    title: `${market.title} — prophetica`,
    description: market.brief,
  };
}

// Deterministic order-book preview derived from the market's quoted bid/ask.
// Replaces real depth data once SPEC-003 wire-up lands; identical price-grid
// math so the visual rhythm carries over.
function makeOrderBook(yesBidCents: number, yesAskCents: number) {
  const baseQty = [1840, 2310, 980, 1520, 740];
  const askQty = [2050, 1670, 1130, 880, 1240];
  return {
    bids: baseQty.map((qty, i) => ({ price: yesBidCents - i, qty })).filter((r) => r.price > 0),
    asks: askQty.map((qty, i) => ({ price: yesAskCents + i, qty })).filter((r) => r.price < 100),
  };
}

interface MockTrade {
  side: "YES" | "NO";
  priceCents: number;
  qty: number;
  ago: string;
}

function makeRecentTrades(market: Market): MockTrade[] {
  const y = market.yesPriceCents;
  return [
    { side: "YES", priceCents: y, qty: 120, ago: "12 秒前" },
    { side: "NO", priceCents: 100 - y - 1, qty: 80, ago: "48 秒前" },
    { side: "YES", priceCents: y - 1, qty: 240, ago: "1 分鐘前" },
    { side: "NO", priceCents: 100 - y, qty: 50, ago: "3 分鐘前" },
    { side: "YES", priceCents: y + 1, qty: 60, ago: "5 分鐘前" },
    { side: "YES", priceCents: y, qty: 320, ago: "12 分鐘前" },
  ];
}

export default async function MarketDetailPage({ params }: PageProps) {
  const { id } = await params;
  const market = getMarketById(id);
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

      {isOpen ? <OpenMarketBody market={market} /> : null}

      <MarketInfoBlock market={market} />
    </>
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
          "mt-2 font-display text-[56px] font-semibold leading-none tracking-[-0.28px] " +
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

function OpenMarketBody({ market }: { market: Market }) {
  const book = makeOrderBook(market.yesBidCents, market.yesAskCents);
  const trades = makeRecentTrades(market);

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
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

function OrderBookTable({ book }: { book: ReturnType<typeof makeOrderBook> }) {
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
                <tr key={`bid-${row.price}`} className="border-b border-proph-divider-soft last:border-0">
                  <td className="py-2 text-proph-primary">{row.price}¢</td>
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
                <tr key={`ask-${row.price}`} className="border-b border-proph-divider-soft last:border-0">
                  <td className="py-2 text-proph-ink">{row.price}¢</td>
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

function RecentTradesTable({ trades }: { trades: MockTrade[] }) {
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

function TradePanel({ market }: { market: Market }) {
  return (
    <aside
      aria-label="下單面板"
      className="self-start rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 lg:sticky lg:top-[140px]"
    >
      <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">下單</h2>
      <p className="mt-1 font-text text-[12px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
        SPEC-003 撮合送單前須通過責任博弈評分（SPEC-006 §4）與 KYC 層級檢查。
      </p>

      <form action="#" aria-disabled="true" className="mt-5 space-y-5">
        <fieldset>
          <legend className="sr-only">選擇方向</legend>
          <div className="grid grid-cols-2 gap-2">
            <label className="cursor-pointer rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary has-[:checked]:bg-proph-canvas-parchment">
              <input type="radio" name="side" value="YES" defaultChecked className="sr-only" />
              <span className="block font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                BUY
              </span>
              <span className="mt-1 block font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-primary">
                YES
              </span>
              <span className="mt-1 block font-mono text-[14px] tabular-nums text-proph-ink-muted-80">
                {market.yesAskCents}¢
              </span>
            </label>
            <label className="cursor-pointer rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary has-[:checked]:bg-proph-canvas-parchment">
              <input type="radio" name="side" value="NO" className="sr-only" />
              <span className="block font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                BUY
              </span>
              <span className="mt-1 block font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-ink">
                NO
              </span>
              <span className="mt-1 block font-mono text-[14px] tabular-nums text-proph-ink-muted-80">
                {100 - market.yesBidCents}¢
              </span>
            </label>
          </div>
        </fieldset>

        <label className="block">
          <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            價格（cent）
          </span>
          <input
            type="number"
            inputMode="numeric"
            min={1}
            max={99}
            defaultValue={market.yesAskCents}
            className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          />
        </label>

        <label className="block">
          <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            數量（股）
          </span>
          <input
            type="number"
            inputMode="numeric"
            min={1}
            defaultValue={10}
            className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          />
        </label>

        <div className="flex items-baseline justify-between border-t border-proph-divider-soft pt-4">
          <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            最大成本
          </span>
          <span className="font-mono text-[21px] font-semibold tabular-nums text-proph-ink">
            {(market.yesAskCents * 10) / 100} 點
          </span>
        </div>

        <button
          type="submit"
          disabled
          className="w-full inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
        >
          送出訂單（後端待接）
        </button>

        <p className="font-text text-[12px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          下單前請確認你已完成 KYC 層級驗證、設定月儲值上限。違反者由 SPEC-003 撮合層直接拒收。
        </p>
      </form>
    </aside>
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
          <a href="/support/disputes" className="text-proph-primary hover:underline">
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
