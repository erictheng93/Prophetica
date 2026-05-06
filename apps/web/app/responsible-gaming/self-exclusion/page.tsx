import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "自我排除 — 責任博弈 — prophetica",
  description: "中長期或永久停止使用本服務。永久排除不可逆。",
  // Self-exclusion is private to the user; do not index.
  robots: { index: false, follow: false },
};

// SPEC-006 §5.1 — Self-exclusion modes. Cool-off lives on /cooldown; this page
// covers Time-out (6 mo / 1 yr) and Permanent.
const timeoutOptions = [
  {
    value: "6m",
    label: "6 個月",
    body: "適合需要較長休息但仍想保留帳戶的玩家。到期後須主動申請解除，並再經 14 天延長冷靜期。",
  },
  {
    value: "1y",
    label: "1 年",
    body: "深度遠離。同樣到期後須主動申請解除 + 14 天冷靜期。",
  },
];

function ScopeBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">自我排除的範圍</h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {[
            { t: "完全凍結帳戶", b: "登入、儲值、交易、收件全部停止，與冷靜期相同。" },
            { t: "持倉照常結算", b: "未結算的對局照原規則結算，結果存入凍結錢包，依模式處理。" },
            { t: "再註冊偵測", b: "KYC 證件 hash 命中名單即拒絕；同 device fingerprint + IP 修改個資視為可疑。" },
            { t: "跨平台名單", b: "若執照地有跨平台排除制度（如 GAMSTOP），自動同步。台灣目前無此制度。" },
          ].map((item) => (
            <li key={item.t} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.t}</h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.b}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function TimeoutChoice() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            選項一
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">中長期暫停（Time-out）</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            6 個月或 1 年。到期後可申請解除，並有 14 天延長冷靜期確保決定不是衝動。
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <legend className="sr-only">中長期暫停期間</legend>
            {timeoutOptions.map((opt) => (
              <label
                key={opt.value}
                className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas p-5 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
              >
                <span className="flex items-center gap-3">
                  <input type="radio" name="timeout" value={opt.value} className="h-4 w-4 accent-proph-primary" />
                  <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                    {opt.label}
                  </span>
                </span>
                <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {opt.body}
                </span>
              </label>
            ))}
            </fieldset>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <label className="flex items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              <input type="checkbox" required className="h-4 w-4 accent-proph-primary" />
              我了解期間無法登入，且解除後仍有 14 天延長冷靜期。
            </label>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              啟動 Time-out（後端待接）
            </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

function PermanentSection() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
        <header className="mb-10">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-body-muted">
            選項二（不可逆）
          </span>
          <h2 className="mt-1 font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            永久排除。
          </h2>
          <p className="mt-4 font-display text-[24px] font-light leading-[1.5]">
            這個選項無法解除。一旦啟動，你的 KYC 證件號碼將被永久寫入排除名單；未來嘗試以同一身份註冊新帳號將被自動拒絕。
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <legend className="font-display text-[21px] font-semibold tracking-[0.231px]">
              點數處理方式
            </legend>
            <label className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-on-dark/20 p-5 has-[:checked]:border-proph-primary-on-dark">
              <span className="flex items-center gap-3">
                <input type="radio" name="pointsDisposition" value="hold" className="h-4 w-4 accent-proph-primary-on-dark" />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  保留 6 個月
                </span>
              </span>
              <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-muted">
                點數凍結保留 6 個月。屆時若未解除排除（永久排除無法解除），點數依條款處理為失效。
              </span>
            </label>
            <label className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-on-dark/20 p-5 has-[:checked]:border-proph-primary-on-dark">
              <span className="flex items-center gap-3">
                <input type="radio" name="pointsDisposition" value="donate" className="h-4 w-4 accent-proph-primary-on-dark" />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  捐贈合作戒癮機構
                </span>
              </span>
              <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-muted">
                點數依當下價值換算，捐贈至 Compliance 合作之戒癮 / 心理衛生機構。捐贈紀錄可索取。
              </span>
            </label>
            </fieldset>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-on-dark/20 px-5 py-4 font-text text-[14px] leading-[1.43] tracking-[-0.224px]">
            <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary-on-dark" />
            <span>
              我清楚知道**永久排除無法解除**。我不是因為一時情緒、糾紛或賠錢才做這個決定，
              而是經過完整評估之後的選擇。
            </span>
            </label>

            <div className="flex flex-col items-start gap-3 border-t border-proph-on-dark/20 pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-body-muted">
              這個按鈕在後端待接前不會生效。建議先撥打援助專線，或聯繫客服協助評估。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill border border-proph-on-dark/40 bg-transparent px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-dark disabled:cursor-not-allowed disabled:opacity-60"
            >
              啟動永久排除（後端待接）
            </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

export default function SelfExclusionPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="責任博弈 / 自我排除"
        headline="這是一個很大的決定。"
        tagline="自我排除不是冷靜期。它代表你想中長期、甚至永久離開 prophetica。請先讀完範圍、再做選擇。"
      />
      <ScopeBlock />
      <TimeoutChoice />
      <PermanentSection />
    </>
  );
}
