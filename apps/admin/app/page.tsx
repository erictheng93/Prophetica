import { ProphTile } from "@prophetica/ui";

// SPEC-017 §6.3 admin stub — login placeholder. Real admin SSO wires through
// SPEC-010 §3.3 (Operator role + IdP federation) once that lands.
//
// Compliance posture
//   - Page itself reveals nothing about platform internals; copy is pure
//     "you've reached an internal console, identify yourself"
//   - robots: noindex (set in layout.tsx)
//   - middleware.ts blocks anything that doesn't carry the internal-network
//     mTLS marker (placeholder until ingress supplies the real header)

export default function AdminLoginPage() {
  return (
    <main className="min-h-screen bg-proph-canvas-parchment">
      <p className="sr-only">prophetica admin - bootstrapping</p>
      <ProphTile
        surface="light"
        kicker="prophetica · admin"
        headline={
          <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            內部營運控制台
          </span>
        }
        tagline="本控制台僅限內部 IP + mTLS 通過後使用。請以 SSO 帳號登入；對外存取一律拒絕。"
      />

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-md px-6 py-16 md:px-10">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              SSO 登入
            </h2>
            <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              admin 帳號由 IT 透過 SSO IdP 配發。SPEC-010 §3.3 實作上線前，本頁僅作為佔位。
            </p>

            <form action="#" aria-disabled="true" className="mt-6 space-y-4">
              <label className="block">
                <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                  公司 Email
                </span>
                <input
                  type="email"
                  autoComplete="username"
                  placeholder="you@prophetica.example"
                  className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                />
              </label>
              <button
                type="submit"
                disabled
                className="w-full inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                以 SSO 繼續（後端待接）
              </button>
            </form>

            <p className="mt-6 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
              所有操作寫入 audit_logs。未授權嘗試會通報 Security 與 Compliance。
              無內部存取需求請離開此頁。
            </p>
          </article>
        </div>
      </section>
    </main>
  );
}
