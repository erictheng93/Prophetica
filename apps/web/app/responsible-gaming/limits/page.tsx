import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "儲值上限 — 責任博弈 — prophetica",
  description: "查看與調整你的月儲值上限。降低立即生效；提高需 24 小時冷卻期。",
};

// SPEC-006 §3.1 — fixed monthly limit options. "無上限" is intentionally
// absent: §3.1 forbids it as a default, and we choose not to surface it at
// all in the shell. Custom amounts must still be ≤ the licensing-region cap
// (TBD per Compliance Officer; not yet wired here).
const limitOptions = [
  { value: 1000, label: "TWD 1,000" },
  { value: 5000, label: "TWD 5,000" },
  { value: 30000, label: "TWD 30,000" },
  { value: 100000, label: "TWD 100,000" },
];

const mockState = {
  currentLimit: 30000,
  thisMonthUsed: 8400,
  lastChangeAt: "2026-04-12",
};

const formatPoints = (n: number) => `${n.toLocaleString("zh-Hant")} 點`;

function CurrentLimitCard() {
  const usagePct = Math.min(100, (mockState.thisMonthUsed / mockState.currentLimit) * 100);
  // SPEC-006 §7.2 — UI must signal at 80% threshold; render warning copy when crossed.
  const nearLimit = usagePct >= 80;

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            目前的月儲值上限
          </div>
          <div className="mt-2 font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-none tracking-[-0.28px]">
            {formatPoints(mockState.currentLimit)}
          </div>
          <div className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            本月已使用 {formatPoints(mockState.thisMonthUsed)}（{usagePct.toFixed(0)}%）。
            上次調整於 {mockState.lastChangeAt}。
          </div>

          <div className="mt-6 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
            <div className="h-full bg-proph-primary" style={{ width: `${usagePct}%` }} />
          </div>

          {nearLimit ? (
            <p className="mt-6 rounded-proph-md border border-proph-primary bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-primary">
              你已使用月儲值上限的 {usagePct.toFixed(0)}%。若已接近上限，請考慮先暫停儲值，
              本月不再加碼。
            </p>
          ) : null}
        </article>
      </div>
    </section>
  );
}

function AdjustForm() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">調整月儲值上限</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            降低立即生效。提高需 24 小時冷卻期，且須過去 30 日無風險訊號。
          </p>
        </header>

        {/*
         * Server-rendered radio form — no submit handler yet. ADR-0004 §9 #6
         * mandates React Hook Form + Zod for any form that actually posts;
         * convert when wallet-ledger-service exposes the limit-change endpoint.
         */}
        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <legend className="sr-only">選擇新的月儲值上限</legend>
            {limitOptions.map((opt) => (
              <label
                key={opt.value}
                className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 transition-colors has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
              >
                <input
                  type="radio"
                  name="monthlyLimit"
                  value={opt.value}
                  defaultChecked={opt.value === mockState.currentLimit}
                  className="h-4 w-4 accent-proph-primary"
                />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {opt.label}
                </span>
              </label>
            ))}
            <label className="flex cursor-pointer flex-col gap-2 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment md:col-span-2">
              <span className="flex items-center gap-3">
                <input type="radio" name="monthlyLimit" value="custom" className="h-4 w-4 accent-proph-primary" />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">自訂金額</span>
              </span>
              <input
                type="number"
                inputMode="numeric"
                min={1}
                placeholder="例如：12,000"
                className="ml-7 w-full max-w-xs rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                aria-label="自訂月儲值上限金額"
              />
            </label>
            </fieldset>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              提交後若為提高上限，將進入 24 小時冷卻期，期間可隨時取消申請。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              送出申請（後端待接）
            </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

export default function LimitsPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="責任博弈 / 限額"
        headline="月儲值上限是你的保護線。"
        tagline="設定一個你能承受的金額，讓 prophetica 在你超過這個額度前主動把門關上。"
      />
      <CurrentLimitCard />
      <AdjustForm />
    </>
  );
}
