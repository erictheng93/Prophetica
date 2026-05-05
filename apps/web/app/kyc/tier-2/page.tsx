import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "Tier 2 標準驗證 — KYC — prophetica",
  description: "證件 OCR + 人臉比對 + Liveness。透過 SumSub Web SDK 完成，2-15 分鐘審核。",
  robots: { index: false, follow: false },
};

// SPEC-005 §2.1.1 — declared_monthly_income optional at Tier 2 (NULL allowed
// per fallback rule). UI presents 4 income brackets (區間中位數 mapping is
// done server-side — UI shows ranges only).
const incomeBuckets = [
  { value: "lt_30k", label: "< TWD 30,000" },
  { value: "30k_60k", label: "TWD 30,000 – 60,000" },
  { value: "60k_100k", label: "TWD 60,000 – 100,000" },
  { value: "gt_100k", label: "> TWD 100,000" },
];

function RequirementsBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">你需要準備</h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {[
            {
              t: "一張有效證件",
              b: "護照、駕照或國民身份證。請確認證件未過期。",
            },
            {
              t: "可以拍照的鏡頭",
              b: "桌機 webcam、筆電前鏡頭或手機鏡頭皆可。Liveness 偵測會請你眨眼或轉頭。",
            },
            {
              t: "地址證明（一張）",
              b: "近 3 個月內的水電 / 銀行 / 政府文件。檔案格式 PDF 或 JPG，大小 < 10 MB。",
            },
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

function SDKPlaceholder() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">啟動 SumSub 驗證</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            點擊下方開始後，將載入 SumSub Web SDK iframe。流程包含證件拍攝 → 自拍 → Liveness 偵測 →
            地址證明上傳。
          </p>
        </header>

        {/*
         * Placeholder for the SumSub Web SDK iframe (SPEC-005 §3.2). Real
         * integration requires:
         *   1. Backend kyc-service exchanges short-lived applicant token via
         *      SumSub API and returns to the client.
         *   2. <SumsubWebSdk accessToken={...} onMessage={...} /> mounts here.
         *   3. webhook /api/internal/webhooks/sumsub flips DB tier on review
         *      result (§3.3).
         * Frontend integration tracked under ADR-0004 §10 V-1.
         */}
        <div className="flex h-[480px] flex-col items-center justify-center gap-3 rounded-proph-lg border-2 border-dashed border-proph-hairline bg-proph-canvas-parchment text-center">
          <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            SumSub Web SDK iframe 載入位置
          </span>
          <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink-muted-80">
            後端串接後 SDK 將在此渲染
          </span>
          <button
            type="button"
            disabled
            className="mt-2 inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
          >
            開始 SumSub 流程（後端待接）
          </button>
        </div>
      </div>
    </section>
  );
}

function IncomeDeclaration() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            可選
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">月收入申報</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            申報後可解鎖更高的月儲值上限（TWD 300,000）。**不申報也可以使用 Tier 2**，但月儲值上限維持 TWD 100,000。
            申報資料視為敏感個資，僅供責任博弈評分使用，客服只看區間不看精確值。
          </p>
        </header>

        <form action="#" aria-disabled="true" className="space-y-6">
          <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <legend className="sr-only">月收入區間</legend>
            {incomeBuckets.map((b) => (
              <label
                key={b.value}
                className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
              >
                <input type="radio" name="incomeBucket" value={b.value} className="h-4 w-4 accent-proph-primary" />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">{b.label}</span>
              </label>
            ))}
            <label className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment md:col-span-2">
              <input type="radio" name="incomeBucket" value="skip" className="h-4 w-4 accent-proph-primary" />
              <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                不申報
              </span>
              <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                月儲值上限維持 TWD 100,000
              </span>
            </label>
          </fieldset>

          <div className="flex justify-end">
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              儲存（後端待接）
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default function Tier2Page() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="KYC / Tier 2 標準"
        headline="證件 + 自拍 + Liveness，2-15 分鐘審核。"
        tagline="本層級透過 SumSub Web SDK 完成。流程在獨立 iframe 內進行，prophetica 不接觸你的證件影像；審核結果以 webhook 回傳後端，並更新你的帳戶層級。"
      />
      <RequirementsBlock />
      <SDKPlaceholder />
      <IncomeDeclaration />
    </>
  );
}
