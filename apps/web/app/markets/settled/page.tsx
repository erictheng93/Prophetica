import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { fetchSettledMarkets } from "@/lib/market-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("markets.settled.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function MarketsSettledPage() {
  const [{ markets }, t] = await Promise.all([
    fetchSettledMarkets(),
    getTranslations("markets.settled"),
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
          {markets.length === 0 ? (
            <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("emptyState")}
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
