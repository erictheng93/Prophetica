import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { fetchNewestMarkets } from "@/lib/market-api";

export const metadata: Metadata = {
  title: "新上架 — prophetica",
  description: "最近開盤的預測市場。",
};

export default async function MarketsNewPage() {
  const { markets } = await fetchNewestMarkets();

  return (
    <>
      <ProphTile
        surface="light"
        kicker="市場 / 新上架"
        headline="剛開放的對局。"
        tagline="這些市場都通過 SPEC-015 雙簽審查，並在過去數天內開盤。流動性可能還在累積。"
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
