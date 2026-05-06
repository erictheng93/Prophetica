import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { listSettled } from "@/lib/mock-markets";

export const metadata: Metadata = {
  title: "已結算 — prophetica",
  description: "近期完成 Oracle 結算的市場。",
};

export default function MarketsSettledPage() {
  const markets = listSettled();

  return (
    <>
      <ProphTile
        surface="light"
        kicker="市場 / 已結算"
        headline="結果出爐的對局。"
        tagline="已通過 Oracle 判定並結算的市場。所有玩家持倉於結算當下入帳，可在個人持倉頁查看。"
      />

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          {markets.length === 0 ? (
            <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              目前沒有已結算市場可顯示。
            </p>
          ) : (
            <div className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
              {markets.map((m) => (
                <MarketCard key={m.id} market={m} />
              ))}
            </div>
          )}
        </div>
      </section>
    </>
  );
}
