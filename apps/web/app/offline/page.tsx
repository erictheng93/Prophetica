import type { Metadata } from "next";
import Link from "next/link";
import { getTranslations } from "next-intl/server";

/*
 * Offline fallback rendered by the service worker when an HTML
 * navigation cannot reach the network. Kept intentionally tiny:
 *   - inline-only CSS via Tailwind utility classes (already cached)
 *   - no client component, no React state, no network fetches
 *   - one CTA back to "/" so the user can retry once they recover
 *
 * SPEC-002 funds-safety: we do NOT show cached prices, balances, or
 * order state on this page. Stale financial values are worse than an
 * honest "你目前處於離線狀態" message.
 */

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("offline.metadata");

  return {
    title: t("title"),
    robots: { index: false, follow: false },
  };
}

export default async function OfflinePage() {
  const t = await getTranslations("offline");

  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas-parchment px-6 py-16">
      <div className="w-full max-w-[480px] rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-8 text-center shadow-proph-card sm:p-12">
        <h1 className="mt-3 font-display text-[34px] font-semibold leading-[1.1] tracking-[-0.374px] text-proph-ink md:text-[40px] md:leading-[1.07]">
          {t("headline")}
        </h1>
        <p className="mt-4 font-text text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
          {t("body")}
        </p>
        <div className="mt-8 flex flex-col items-center gap-3 sm:flex-row sm:justify-center">
          <Link
            href="/"
            className="inline-flex h-12 min-w-[160px] items-center justify-center rounded-proph-pill bg-proph-primary px-6 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary shadow-proph-button transition-all duration-150 hover:bg-proph-primary-focus active:scale-[0.98]"
          >
            {t("retry")}
          </Link>
        </div>
      </div>
    </main>
  );
}
