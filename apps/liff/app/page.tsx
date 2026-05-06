import { LiffBootstrap } from "@/components/liff-bootstrap";

// SPEC-017 §6.3 liff stub
//   - Loads LIFF SDK and calls liff.init({ liffId: NEXT_PUBLIC_LIFF_ID })
//   - Fail-fast when NEXT_PUBLIC_LIFF_ID is missing
//
// Real liff app surfaces (browse markets, top up, view positions) reuse
// apps/web's UI primitives via @prophetica/ui — but liff-side flow is
// LINE-friendly: vertical layout, no top nav (the LINE bar replaces it),
// LIFF identity instead of email/password.

export default function LiffEntryPage() {
  // process.env reads in Next.js are inlined at build time for
  // NEXT_PUBLIC_* — runtime mutation is not possible.
  const liffId = process.env.NEXT_PUBLIC_LIFF_ID ?? "";

  if (!liffId) {
    return <MissingLiffIdScreen />;
  }

  return <LiffBootstrap liffId={liffId} />;
}

function MissingLiffIdScreen() {
  return (
    <main className="min-h-screen bg-proph-canvas-parchment">
      <p className="sr-only">prophetica liff - bootstrapping</p>
      <section className="mx-auto flex min-h-screen max-w-md flex-col justify-center px-6 py-12">
        <div className="rounded-proph-lg border border-proph-feedback-error bg-proph-canvas p-6">
          <h1 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-feedback-error">
            設定不完整
          </h1>
          <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <code className="rounded-proph-sm border border-proph-divider-soft bg-proph-canvas-parchment px-1.5 py-0.5 font-mono text-[12px]">
              NEXT_PUBLIC_LIFF_ID
            </code>{" "}
            環境變數未設定。LIFF mini app 無法啟動。
          </p>
          <p className="mt-3 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
            開發者：請依 SPEC-017 §7.1 在 <code className="font-mono">.env</code> 補
            <code className="font-mono"> NEXT_PUBLIC_LIFF_ID</code>，再重啟 dev server。
            正式部署時由 Cloudflare Pages 環境變數注入。
          </p>
        </div>
      </section>
    </main>
  );
}
