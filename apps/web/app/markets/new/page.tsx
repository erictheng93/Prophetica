import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { fetchNewestMarkets } from "@/lib/market-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("markets.new.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function MarketsNewPage() {
  const [{ markets }, t] = await Promise.all([
    fetchNewestMarkets(),
    getTranslations("markets.new"),
  ]);

  return (
    <>
      <ProphTile
        surface="light"
        headline={t("headline")}
        tagline={t("tagline")}
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
