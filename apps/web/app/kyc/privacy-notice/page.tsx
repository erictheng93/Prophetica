import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

// SPEC-005 §8.1 mandate — "採集前必有單獨頁面說明：我們收什麼、為什麼、保存多久"。
// This page satisfies that requirement. It must be linked from every KYC
// form (Tier 1 / 2 / 3) and read-acknowledged before submission.

export const metadata: Metadata = {
  title: "KYC 隱私說明 — prophetica",
  description: "我們收哪些 KYC 欄位、為什麼收、保存多久、誰可以存取。",
  robots: { index: true, follow: true },
};

interface PrivacyRow {
  field: string;
  collected: string;
  purpose: string;
  retention: string;
  access: string;
}

const matrix: PrivacyRow[] = [
  {
    field: "姓名（與證件一致）",
    collected: "Tier 1 / 2 / 3",
    purpose: "建立法律意義上的帳戶身份；制裁名單比對；結算憑證。",
    retention: "帳戶生命週期 + 7 年（依 SPEC-001 §1.1 ③ 稽核留痕要求）。",
    access: "你本人 + 通過合規培訓的客服 + Compliance Officer + 受監管稽核機構。",
  },
  {
    field: "生日",
    collected: "Tier 1 / 2 / 3",
    purpose: "驗證 ≥ 18 歲（SPEC-006 §6 未成年保護）。",
    retention: "同帳戶生命週期 + 7 年。",
    access: "同上。",
  },
  {
    field: "證件號碼",
    collected: "Tier 1（隱式驗證）/ Tier 2 / 3",
    purpose: "唯一性比對、制裁名單比對、self-exclusion 名單比對（SPEC-006 §5.4）。",
    retention: "**只儲存 SHA256 雜湊**，原始號碼不入庫，提交後立即丟棄。雜湊保存同帳戶生命週期 + 7 年。",
    access: "後端 kyc-service（只能比對，不能還原原始號碼）。",
  },
  {
    field: "證件影像",
    collected: "Tier 2 / 3（透過 SumSub）",
    purpose: "OCR 抽取資訊；審核時的真偽判定。",
    retention: "SumSub Cloud（EU region）+ 我方 R2（EU）每日批次副本，AES-256 at-rest 加密，Object Lock 7 年。",
    access: "Compliance Officer + SumSub 審核員 + 受監管稽核機構。客服無法直接存取原始影像。",
  },
  {
    field: "自拍 + Liveness",
    collected: "Tier 2 / 3",
    purpose: "與證件影像比對人臉一致性。Liveness 防止以照片冒充本人。",
    retention: "同證件影像。",
    access: "同證件影像。",
  },
  {
    field: "地址證明",
    collected: "Tier 2 / 3",
    purpose: "驗證居住地與聲明地一致；地理風險評估。",
    retention: "同證件影像。",
    access: "同證件影像。",
  },
  {
    field: "月收入（區間）",
    collected: "Tier 2 自願 / Tier 3 必填",
    purpose: "責任博弈評分（SPEC-006 §4.1）；解鎖更高儲值上限。",
    retention: "同帳戶生命週期；12 個月未更新自動標記過時。",
    access: "你本人 + Compliance Officer。**客服只看區間，不看精確值**。",
  },
  {
    field: "收入來源證明 / 銀行對帳單",
    collected: "Tier 3",
    purpose: "驗證申報月收入；AML 規則引擎。",
    retention: "R2 加密儲存 7 年。",
    access: "Compliance Officer + 受監管稽核機構。",
  },
];

function PrivacyMatrix() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">逐欄說明</h2>

        <div className="space-y-4">
          {matrix.map((row) => (
            <article
              key={row.field}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <header className="flex flex-wrap items-baseline gap-3">
                <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {row.field}
                </h3>
                <span className="rounded-proph-pill border border-proph-hairline bg-proph-canvas-parchment px-3 py-1 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {row.collected}
                </span>
              </header>

              <dl className="mt-4 grid grid-cols-1 gap-4 md:grid-cols-3">
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    為什麼
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {row.purpose}
                  </dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    保存多久
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {row.retention}
                  </dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    誰可以存取
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {row.access}
                  </dd>
                </div>
              </dl>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function YourRights() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-6 font-display text-[40px] font-semibold leading-[1.1]">你的權利</h2>
        <p className="max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          完整的個資權利說明在
          <a href="/legal/privacy" className="text-proph-primary hover:underline">
            隱私政策
          </a>
          。摘要如下：
        </p>
        <ul className="mt-6 space-y-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          <li>· **查詢權**：可隨時匯出所有 KYC 欄位（24 小時內生成 zip）</li>
          <li>· **更正權**：姓名、地址等可申請更正；敏感欄位（生日 / 證件號）需重新 KYC</li>
          <li>· **刪除權**：軟刪除帳戶；KYC 紀錄依法律要求最少保留 7 年</li>
          <li>· **攜帶權**：匯出格式為 JSON，可攜至其他服務</li>
          <li>· **反對權**：行銷推播 opt-out；KYC 處理為履約必要，無法反對</li>
        </ul>
      </div>
    </section>
  );
}

export default function PrivacyNoticePage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="KYC 隱私說明"
        headline="先告訴你我們收什麼、為什麼、保存多久。"
        tagline="這份說明是依 SPEC-005 §8.1 設計：在你提交任何 KYC 資料之前，先把流程透明攤開。每個欄位的目的、保存期限、存取者一覽。"
      />
      <PrivacyMatrix />
      <YourRights />
    </>
  );
}
