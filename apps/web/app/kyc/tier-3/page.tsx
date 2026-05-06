import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "Tier 3 強化驗證 — KYC — prophetica",
  description: "高活動量帳戶的 EDD（Enhanced Due Diligence）。需提交收入證明、銀行對帳單、視訊驗證。",
  robots: { index: false, follow: false },
};

// SPEC-005 §2.1 + §6.3 — Tier 3 has higher friction. PEP-positive accounts
// are auto-routed here; UI must avoid implying the user is "suspected" of
// anything. Frame as "additional verification when activity exceeds Tier 2."

function WhyHere() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="font-display text-[40px] font-semibold leading-[1.1]">為什麼會走到這一層？</h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          下列任一情況都會讓你進入 Tier 3 流程：
        </p>
        <ul className="mt-6 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          <li>· 月儲值已達 Tier 2 上限（TWD 300,000），需要更高額度</li>
          <li>· 單筆儲值超過 USD 10,000 等值</li>
          <li>· 制裁名單 / PEP 比對命中（依當地法規處理）</li>
          <li>· Compliance 發起的個案審核</li>
          <li>· 玩家主動申請以解鎖更高活動空間</li>
        </ul>
      </div>
    </section>
  );
}

function Requirements() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">需要提交的文件</h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {[
            {
              kicker: "必填",
              t: "收入來源證明",
              b: "薪資單、扣繳憑單、租金 / 投資收入紀錄。文件須涵蓋過去 3 個月，金額需與申報之月收入一致（誤差 ±15%）。",
            },
            {
              kicker: "必填",
              t: "銀行對帳單",
              b: "近 3 個月主要往來帳戶。可遮蔽帳號末 4 碼以外的所有數字，但日期、金額、餘額不得遮蔽。",
            },
            {
              kicker: "必填",
              t: "視訊驗證",
              b: "由 iProov 提供的 1 對 1 視訊驗證，3-5 分鐘。預約時段後會收到加密連結。視訊不留存影像，僅留存通過 / 未通過判定。",
            },
            {
              kicker: "選填",
              t: "PEP / 制裁名單聲明",
              b: "若你或近親為政治公眾人物（PEP），請主動聲明以加速 Compliance 審核。系統亦會自動比對 OFAC / UN / EU / OFSI 等名單。",
            },
          ].map((item) => (
            <article
              key={item.t}
              className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {item.kicker}
              </span>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {item.t}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.b}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function StartFlow() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">啟動 Tier 3 流程</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            按下開始後，系統將為你建立 Compliance 工單並寄送加密文件上傳連結到帳號 Email。
            審核期間你的帳戶**不會**被凍結，可繼續以目前 Tier 2 上限進行交易。
          </p>
        </header>

        <div className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-8">
          <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            審核時程預估
          </h3>
          <ul className="mt-4 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            <li>· 文件提交：你的時程</li>
            <li>· 視訊預約：1-3 個工作天</li>
            <li>· Compliance 審核：5-10 個工作天</li>
            <li>· 結果通知：站內訊息 + Email</li>
          </ul>

          <div className="mt-8 flex flex-col items-start gap-3 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              審核期間若有疑問，可由
              <a href="/support/contact" className="text-proph-primary underline">
                客服管道
              </a>
              聯繫專屬 Compliance 窗口。
            </p>
            <button
              type="button"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              建立 Compliance 工單（後端待接）
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Tier3Page() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="KYC / Tier 3 強化"
        headline="高活動量需要更深一層的驗證。"
        tagline="Tier 3 適用於月活動量超過 Tier 2 上限的玩家、或受平台 Compliance 主動觸發的個案。流程比較長，但你的帳戶在審核期間保持可用。"
      />
      <WhyHere />
      <Requirements />
      <StartFlow />
    </>
  );
}
