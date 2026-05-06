import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { listTrending } from "@/lib/mock-markets";

export const metadata: Metadata = {
  title: "熱門市場 — prophetica",
  description: "依 24 小時成交量排序的熱門對局。",
};

export default function MarketsTrendingPage() {
  const markets = listTrending();

  return (
    <>
      <ProphTile
        surface="light"
        kicker="市場 / 熱門"
        headline="玩家最關注的對局。"
        tagline="依過去 24 小時成交量排序。成交量大代表市場意見正在收斂，但不必然代表共識正確。"
      />

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
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
