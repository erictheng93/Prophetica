import type { Metadata } from "next";
import { ArrowUpRight, Warning } from "@phosphor-icons/react/dist/ssr";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "申訴流程 — prophetica",
  description: "結算爭議、KYC 異議、安全事件、技術故障的申訴流程與時程。",
};

// SPEC-012 §3.1 ticket categories — kept in sync with backend ticket router.
const categories = [
  {
    id: "settlement",
    label: "結算爭議",
    priority: "高",
    sla: "24 小時內首次回應",
    body: "對某個市場的 Oracle 判定有疑問。請附市場 ID 與你認為應為的結果。SPEC-004 §5 規定 24 小時申訴窗口；逾期受理但裁決機率降低。",
  },
  {
    id: "kyc",
    label: "KYC 異議",
    priority: "標準",
    sla: "24 小時內首次回應",
    body: "Tier 升級被拒、SumSub 流程多次失敗、證件被誤判。客服會在審核後安排重啟流程或人工 review。",
  },
  {
    id: "topup",
    label: "儲值問題",
    priority: "高",
    sla: "24 小時內首次回應",
    body: "點數未入帳、支付被拒、月儲值上限相關問題。請附訂單編號與支付通路。",
  },
  {
    id: "security",
    label: "安全事件",
    priority: "緊急",
    sla: "4 小時內優先回應",
    body: "帳號被盜、可疑登入、SMS 被攔截。客服會立即凍結帳戶並啟動 Security 審查。",
  },
  {
    id: "technical",
    label: "技術故障",
    priority: "標準",
    sla: "24 小時內首次回應",
    body: "頁面錯誤、API 失敗、資料顯示異常。請描述重現步驟與時間。",
  },
  {
    id: "rg",
    label: "責任博弈個案",
    priority: "高",
    sla: "4 小時內回撥（電話）",
    body: "玩家或親友覺得遊戲節奏失控。受過認證的 RG 客服處理；不會以親友通報為據強制停權。",
  },
];

const flow = [
  { state: "NEW", label: "工單建立", body: "你提交後立即進入 NEW 狀態，系統自動分配工單 ID。" },
  { state: "ASSIGNED", label: "已分派", body: "T1 / T2 / T3 客服依分類接案，通常 1 小時內。" },
  { state: "IN_PROGRESS", label: "處理中", body: "客服查證中。期間可能在工單留言請你補件。" },
  { state: "AWAITING_USER", label: "等你回覆", body: "客服需要你提供更多資訊；逾期未回應工單會自動關閉。" },
  { state: "RESOLVED", label: "已解決", body: "客服認定問題已處理，等你 confirm。" },
  { state: "CLOSED", label: "已結案", body: "你 confirm 或自動 7 日後關閉。後續若有相同問題請新開工單。" },
];

function CategoryGrid() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">分類與優先級</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            分類影響工單路由與 SLA。安全事件最優先；結算 / 儲值 / RG 為高優先；其餘為標準。
          </p>
        </header>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {categories.map((c) => (
            <li key={c.id} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <div className="flex items-baseline justify-between gap-3">
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{c.label}</h3>
                <span
                  className={
                    "inline-flex items-center gap-1 rounded-proph-pill border px-3 py-0.5 font-text text-[12px] uppercase tracking-[-0.12px] " +
                    (c.priority === "緊急"
                      ? "border-proph-feedback-error text-proph-feedback-error"
                      : c.priority === "高"
                      ? "border-proph-primary text-proph-primary"
                      : "border-proph-divider-soft text-proph-ink-muted-48")
                  }
                >
                  {c.priority === "緊急" ? (
                    <Warning weight="fill" className="h-3 w-3 shrink-0" aria-hidden="true" />
                  ) : c.priority === "高" ? (
                    <ArrowUpRight weight="bold" className="h-3 w-3 shrink-0" aria-hidden="true" />
                  ) : null}
                  {c.priority}
                </span>
              </div>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-primary">
                {c.sla}
              </div>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function FlowTimeline() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">工單會經過什麼？</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            SPEC-012 §3.2 工單狀態機。每次狀態變動會在工單留言串通知你。
          </p>
        </header>
        <ol className="space-y-3">
          {flow.map((step, i) => (
            <li
              key={step.state}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[12px] tabular-nums tracking-[-0.12px] text-proph-ink-muted-48">
                  {String(i + 1).padStart(2, "0")}
                </span>
                <code className="rounded-proph-sm border border-proph-divider-soft bg-proph-canvas-parchment px-2 py-0.5 font-mono text-[12px] tracking-[-0.12px] text-proph-ink">
                  {step.state}
                </code>
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{step.label}</h3>
              </div>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {step.body}
              </p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function PrivacyCallout() {
  // SPEC-012 §5.2 — be transparent about what CS can and cannot see.
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-6">
          <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">客服在你提交申訴後看得到什麼？</h2>
          <div className="mt-4 grid grid-cols-1 gap-6 md:grid-cols-2">
            <div>
              <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-success">
                客服可查
              </h3>
              <ul className="mt-2 space-y-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                <li>· 你提交的工單內容、附件、時間戳</li>
                <li>· 你的 KYC tier、最近 30 日工單、儲值 / 交易紀錄</li>
                <li>· 證件號碼**末 4 碼 hash**（用於唯一性比對，無法還原）</li>
                <li>· 你的 RG 評分歷史</li>
              </ul>
            </div>
            <div>
              <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-error">
                客服不可查
              </h3>
              <ul className="mt-2 space-y-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                <li>· 你的證件影像（須 Compliance 簽核）</li>
                <li>· 你的完整身份證號 / 信用卡號</li>
                <li>· 你的密碼（任何形式）</li>
                <li>· 你與其他平台的活動</li>
              </ul>
            </div>
          </div>
          <p className="mt-4 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
            每筆客服查詢都會寫入 audit_logs，包含操作員 ID、時間、動作類型，可被合規與監管稽核。
          </p>
        </article>
      </div>
    </section>
  );
}

function SubmissionForm() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">提交申訴</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            送出後即時收到工單 ID。同個案件請使用同一工單追蹤，避免重複建立。
          </p>
        </header>

        <form action="#" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              分類
            </span>
            <select
              name="category"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            >
              <option value="">— 請選擇分類 —</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}（{c.priority}）
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              相關市場 ID（如有）
            </span>
            <input
              type="text"
              name="marketId"
              placeholder="例如：fomc-2026-06-rate-cut"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              結算 / 交易爭議請務必填寫，可在市場頁網址列複製。
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              詳細說明
            </span>
            <textarea
              name="body"
              rows={6}
              placeholder="請描述發生時間、預期行為、實際行為、你已嘗試的步驟。"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              附件（截圖、PDF；單檔 ≤ 10 MB）
            </span>
            <input
              type="file"
              name="attachments"
              multiple
              className="mt-1 block w-full font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80 file:mr-4 file:cursor-pointer file:rounded-proph-pill file:border-0 file:bg-proph-canvas-parchment file:px-4 file:py-2 file:font-text file:text-[14px] file:tracking-[-0.224px] file:text-proph-ink hover:file:bg-proph-divider-soft"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              請勿上傳證件影像。客服在工單範圍內無法存取證件，亦不會主動要求。
            </span>
          </label>

          <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
            <span>
              我了解工單範圍內的查詢會被記錄於 audit_logs，並接受{" "}
              <a href="/legal/privacy" className="text-proph-primary underline">
                隱私政策
              </a>
              對申訴資料的處理規範。
            </span>
          </label>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              送出後將顯示工單 ID。請保留以便追蹤進度。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              送出申訴（後端待接）
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default function DisputesPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="客服 / 申訴流程"
        headline="說清楚問題，我們會處理。"
        tagline="所有申訴 24 小時內收到狀態回覆（即使尚未裁決）。結算爭議與安全事件由 T2 / T3 客服優先處理。"
        actions={<ProphLinkButton variant="secondary-pill" href="/support/faq">先看常見問題</ProphLinkButton>}
      />
      <CategoryGrid />
      <FlowTimeline />
      <PrivacyCallout />
      <SubmissionForm />
    </>
  );
}
