import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { MarketCard } from "@/components/market-card";
import { fetchOpenMarkets } from "@/lib/market-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("markets.all.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function MarketsAllPage() {
  const [{ markets }, t] = await Promise.all([
    fetchOpenMarkets(),
    getTranslations("markets.all"),
  ]);

  return (
    <>
      <ProphTile
        surface="light"
        glow="brand"
        gridPattern
        headline={t("headline")}
        tagline={t("tagline")}
      />

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8 flex items-baseline justify-between">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("countLabel", { count: markets.length })}
            </h2>
            <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("sortLabel")}
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
