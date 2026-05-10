import type { Metadata } from "next";
import Link from "next/link";

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

export const metadata: Metadata = {
  title: "離線中",
  robots: { index: false, follow: false },
};

export default function OfflinePage() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas-parchment px-6 py-16">
      <div className="w-full max-w-[480px] text-center">
        <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-feedback-warning">
          離線狀態
        </p>
        <h1 className="mt-3 text-[34px] font-semibold leading-[1.1] tracking-[-0.374px] text-proph-ink md:text-[40px] md:leading-[1.07]">
          無法連線到 Prophetica
        </h1>
        <p className="mt-4 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
          檢查 Wi-Fi 或行動網路，然後點擊下方按鈕重試。為避免顯示過期資料，我們不在離線狀態快取餘額或下單畫面。
        </p>
        <div className="mt-8 flex flex-col items-center gap-3 sm:flex-row sm:justify-center">
          <Link
            href="/"
            className="inline-flex h-12 min-w-[160px] items-center justify-center rounded-proph-pill bg-proph-primary px-6 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
          >
            重試
          </Link>
        </div>
      </div>
    </main>
  );
}
