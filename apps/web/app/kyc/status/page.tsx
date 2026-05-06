import type { Metadata } from "next";
import type { Icon } from "@phosphor-icons/react";
import { CheckCircle, Circle, Spinner, XCircle } from "@phosphor-icons/react/dist/ssr";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "審核狀態 — KYC — prophetica",
  description: "查看你的 KYC 提交與審核進度。",
  robots: { index: false, follow: false },
};

// Mock review state. Real timeline comes from kyc-service via packages/ts-sdk.
type StepState = "done" | "current" | "pending" | "failed";

interface TimelineStep {
  label: string;
  detail: string;
  state: StepState;
  timestamp?: string;
}

const mockTimeline: TimelineStep[] = [
  { label: "Tier 1 提交", detail: "證件號碼隱式驗證通過", state: "done", timestamp: "2026-04-12 14:21" },
  { label: "Tier 2 提交", detail: "已上傳證件 + 自拍 + 地址證明", state: "done", timestamp: "2026-05-04 09:08" },
  { label: "SumSub 審核中", detail: "預估完成時間 5-15 分鐘", state: "current", timestamp: "2026-05-04 09:09" },
  { label: "結果通知", detail: "Email + 站內訊息", state: "pending" },
  { label: "帳戶層級更新", detail: "通過後將自動升級至 Tier 2", state: "pending" },
];

const stateColor: Record<StepState, string> = {
  done: "border-proph-ink-muted-80 bg-proph-canvas",
  current: "border-proph-primary bg-proph-canvas-parchment",
  pending: "border-proph-divider-soft bg-proph-canvas",
  failed: "border-proph-feedback-error bg-proph-canvas",
};

const stateLabel: Record<StepState, string> = {
  done: "完成",
  current: "進行中",
  pending: "等待中",
  failed: "失敗",
};

const stateIcon: Record<StepState, Icon> = {
  done: CheckCircle,
  current: Spinner,
  pending: Circle,
  failed: XCircle,
};

const stateIconClass: Record<StepState, string> = {
  done: "text-proph-feedback-success",
  current: "text-proph-primary animate-spin",
  pending: "text-proph-ink-muted-48",
  failed: "text-proph-feedback-error",
};

function Timeline() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">最近一次審核</h2>

        <ol className="space-y-3">
          {mockTimeline.map((step, idx) => {
            const Icon = stateIcon[step.state];
            return (
            <li
              key={`${step.label}-${idx}`}
              className={`rounded-proph-lg border-l-4 border bg-proph-canvas p-5 ${stateColor[step.state]}`}
            >
              <div className="flex flex-wrap items-baseline justify-between gap-3">
                <div className="flex items-center gap-3">
                  <Icon
                    weight="bold"
                    className={`h-5 w-5 shrink-0 ${stateIconClass[step.state]}`}
                    aria-hidden
                  />
                  <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    步驟 {idx + 1} · {stateLabel[step.state]}
                  </span>
                  <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {step.label}
                  </h3>
                </div>
                {step.timestamp ? (
                  <span className="font-mono text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                    {step.timestamp}
                  </span>
                ) : null}
              </div>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {step.detail}
              </p>
            </li>
            );
          })}
        </ol>
      </div>
    </section>
  );
}

function NextSteps() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-6 font-display text-[40px] font-semibold leading-[1.1]">你可以做什麼</h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">繼續以目前 Tier 1 玩</h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              審核期間 Tier 1 額度仍可使用：月 TWD 30,000、單筆 TWD 5,000。
            </p>
            <ProphLinkButton variant="secondary-pill" href="/markets" className="mt-4">
              瀏覽市場
            </ProphLinkButton>
          </article>
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">遇到問題？</h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              審核超過 30 分鐘未完成、或收到 RED 結果不理解，可申訴或聯繫客服。
            </p>
            <ProphLinkButton variant="secondary-pill" href="/support/disputes" className="mt-4">
              申訴流程
            </ProphLinkButton>
          </article>
        </div>
      </div>
    </section>
  );
}

export default function StatusPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="KYC / 審核狀態"
        headline="你的審核進度，一目了然。"
        tagline="所有 KYC 提交與審核步驟在這裡公開追蹤。SumSub 通常 2-15 分鐘完成；完成時會發 Email + 站內訊息，不需停留在這頁等。"
      />
      <Timeline />
      <NextSteps />
    </>
  );
}
