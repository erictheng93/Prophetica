import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { fetchOpenMarkets } from "@/lib/market-api";

export const metadata: Metadata = {
  title: "全部市場 — prophetica",
  description: "瀏覽所有開放中的預測市場。",
};

export default async function MarketsAllPage() {
  const { markets } = await fetchOpenMarkets();

  return (
    <>
      <ProphTile
        surface="light"
        kicker="市場"
        headline="所有開放中的對局。"
        tagline="每個市場都是 YES / NO 二元問題，由 prophetica 中立撮合，玩家以站內點數對結果表態。"
      />

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8 flex items-baseline justify-between">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {markets.length} 個開放中
            </h2>
            <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              排序：依預設順序
            </span>
          </header>
          <div className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
            {markets.map((m) => (
              <MarketCard key={m.id} market={m} />
            ))}
          </div>
        </div>
      </section>
    </>
  );
}
