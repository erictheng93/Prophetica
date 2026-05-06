import type { Metadata } from "next";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  Brand,
  FooterLegalRow,
  legalColumn,
  MainNavLinks,
  supportColumn,
} from "@/lib/site-nav";

export const metadata: Metadata = {
  title: "登入 / 註冊 — prophetica",
  description: "登入既有帳戶，或註冊新帳戶開始預測對局。註冊後會引導你完成 4 個必經設定。",
  robots: { index: false, follow: false },
};

interface PageProps {
  searchParams: Promise<{ mode?: string | string[] }>;
}

const sectionFooterColumns = [legalColumn, supportColumn];

// SPEC-005 §2.4 first-launch 5 countries — kept in sync with /kyc/tier-1.
const launchCountries = [
  { code: "TW", name: "台灣" },
  { code: "JP", name: "日本" },
  { code: "KR", name: "韓國" },
  { code: "VN", name: "越南" },
  { code: "MY", name: "馬來西亞" },
];

// Steps that follow successful sign-up. SPEC-006 §3.1 mandates these as
// non-skippable; SPEC-005 §2.4.1 makes Tier 1 the entry point for credentials.
const postSignupSteps = [
  { num: "01", label: "年齡確認", body: "確認你滿 18 歲。失敗即停止註冊。" },
  { num: "02", label: "責任博弈聲明", body: "閱讀並勾選同意。明列 prophetica 為娛樂遊戲、非投資工具。" },
  { num: "03", label: "月儲值上限設定", body: "TWD 1k / 5k / 30k / 100k 擇一。預設不可選「無上限」。" },
  { num: "04", label: "援助資源連結", body: "顯示 1925 安心專線等外部資源。確認後才進入 KYC Tier 1。" },
];

export default async function AuthPage({ searchParams }: PageProps) {
  const params = await searchParams;
  const raw = Array.isArray(params.mode) ? params.mode[0] : params.mode;
  const mode: "signin" | "signup" = raw === "signup" ? "signup" : "signin";

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={
          <a
            href="/support"
            className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
          >
            登入有問題？
          </a>
        }
      />

      <ProphTile
        surface="light"
        kicker={mode === "signup" ? "註冊新帳戶" : "登入"}
        headline={mode === "signup" ? "5 步驟內完成註冊。" : "歡迎回來。"}
        tagline={
          mode === "signup"
            ? "註冊後會自動進入 4 個必經設定（年齡 → RG 聲明 → 儲值上限 → 援助資源），最後才到 KYC Tier 1。"
            : "用註冊 Email 與密碼登入。第一次登入會提示啟用兩步驗證。"
        }
      />

      <ModeToggle mode={mode} />

      {mode === "signup" ? <SignupForm /> : <SigninForm />}

      {mode === "signup" ? <PostSignupPreview /> : null}

      <ComplianceReminder />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function ModeToggle({ mode }: { mode: "signin" | "signup" }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
        <div role="tablist" aria-label="選擇登入或註冊" className="flex gap-2">
          <a
            role="tab"
            aria-selected={mode === "signin"}
            href="/auth/sign-in"
            className={
              "inline-flex items-center rounded-proph-pill px-5 py-2 font-text text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors " +
              (mode === "signin"
                ? "bg-proph-primary text-proph-on-primary"
                : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary hover:text-proph-ink")
            }
          >
            登入
          </a>
          <a
            role="tab"
            aria-selected={mode === "signup"}
            href="/auth/sign-in?mode=signup"
            className={
              "inline-flex items-center rounded-proph-pill px-5 py-2 font-text text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors " +
              (mode === "signup"
                ? "bg-proph-primary text-proph-on-primary"
                : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary hover:text-proph-ink")
            }
          >
            註冊
          </a>
        </div>
      </div>
    </section>
  );
}

function SigninForm() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action="#" aria-disabled="true" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              Email
            </span>
            <input
              type="email"
              name="email"
              autoComplete="username"
              placeholder="you@example.com"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="flex items-baseline justify-between">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                密碼
              </span>
              <a
                href="/support/contact?topic=password-reset"
                className="font-text text-[14px] tracking-[-0.224px] text-proph-primary hover:underline"
              >
                忘記密碼？
              </a>
            </span>
            <input
              type="password"
              name="password"
              autoComplete="current-password"
              placeholder="••••••••"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="flex items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            <input type="checkbox" name="remember" className="h-4 w-4 accent-proph-primary" />
            在此裝置保持登入 14 天（公用裝置請勿勾選）
          </label>

          <div className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <strong className="font-semibold text-proph-ink">兩步驗證：</strong>{" "}
            首次登入會引導你綁定 TOTP App（Google Authenticator、1Password 等）。
            遺失裝置請聯繫{" "}
            <a href="/support/contact" className="text-proph-primary hover:underline">
              客服
            </a>
            申請重設，需重新通過 KYC。
          </div>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              送出後依設定送 OTP / TOTP；未驗證前不會建立 session。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              登入（後端待接）
            </button>
          </div>

          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            沒有帳戶？{" "}
            <a href="/auth/sign-in?mode=signup" className="text-proph-primary hover:underline">
              註冊一個
            </a>
          </p>
        </form>
      </div>
    </section>
  );
}

function SignupForm() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action="#" aria-disabled="true" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              Email
            </span>
            <input
              type="email"
              name="email"
              autoComplete="email"
              placeholder="you@example.com"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              我們會寄驗證連結到此 Email。也是後續客服聯繫的主要管道。
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              生日
            </span>
            <input
              type="date"
              name="birthday"
              required
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              prophetica 為成年娛樂遊戲。生日 &lt; 18 歲將立即拒絕註冊。
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              所在國家
            </span>
            <select
              name="country"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            >
              <option value="">— 請選擇 —</option>
              {launchCountries.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.name}
                </option>
              ))}
            </select>
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              我們目前只開放這 5 國。其他國家請查看{" "}
              <a href="/legal/geo-block" className="text-proph-primary hover:underline">
                服務地區限制
              </a>
              。
            </span>
          </label>

          <fieldset className="space-y-3">
            <legend className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              你必須了解三件事
            </legend>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
              <span>
                我已閱讀{" "}
                <a href="/legal/terms" className="text-proph-primary hover:underline">
                  服務條款
                </a>{" "}
                與{" "}
                <a href="/legal/privacy" className="text-proph-primary hover:underline">
                  隱私政策
                </a>
                ，並同意 prophetica 依此處理我的資料。
              </span>
            </label>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
              <span>
                我了解 prophetica <strong className="font-semibold text-proph-ink">站內點數不可兌回</strong>
                現金、虛擬貨幣或實體商品。所有儲值僅限站內娛樂使用。
              </span>
            </label>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
              <span>
                我了解 prophetica 為娛樂遊戲，<strong className="font-semibold text-proph-ink">非投資工具</strong>。
                我願意設定月儲值上限作為自我保護機制，並隨時可使用冷靜期 / 自我排除工具。
              </span>
            </label>
          </fieldset>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              送出後將進入 4 步必經流程，最後才開帳戶。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              開始註冊（後端待接）
            </button>
          </div>

          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            已有帳戶？{" "}
            <a href="/auth/sign-in" className="text-proph-primary hover:underline">
              登入
            </a>
          </p>
        </form>
      </div>
    </section>
  );
}

function PostSignupPreview() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">送出後會發生什麼</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            SPEC-006 §3.1 規定的 4 個必經步驟。每一步都不可跳過；中途離開要從頭開始。
          </p>
        </header>

        <ol className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {postSignupSteps.map((s) => (
            <li
              key={s.num}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-primary">
                  {s.num}
                </span>
                <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                  {s.label}
                </h3>
              </div>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {s.body}
              </p>
            </li>
          ))}
        </ol>

        <p className="mt-6 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
          完成這 4 步後進入{" "}
          <a href="/kyc/tier-1" className="text-proph-primary hover:underline">
            KYC Tier 1 基礎驗證
          </a>
          （另 10 秒）。即可瀏覽市場開始下單。
        </p>
      </div>
    </section>
  );
}

function ComplianceReminder() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          請務必記得
        </span>
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
          18 歲以下不得參與。站內點數不可兌回。如已失控，撥打{" "}
          <a href="tel:1925" className="text-proph-primary-on-dark hover:underline">
            1925
          </a>{" "}
          安心專線 24 小時免費。
        </h2>
        <div className="mt-6">
          <ProphLinkButton variant="primary" href="/responsible-gaming/resources">
            責任博弈資源
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}
