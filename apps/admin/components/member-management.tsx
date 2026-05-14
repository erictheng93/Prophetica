"use client";

import { useMemo, useState, type FormEvent, type ReactNode } from "react";
import { StatusPill } from "@/components/status-pill";
import {
  formatMinorUnitsAsMemberPoints,
  memberSearchHints,
  type MemberActionResponse,
  type MemberActionType,
  type MemberManagementSummary,
  type MemberStatus,
  type RiskLevel,
} from "@/lib/member-management-shared";
import type { Role } from "@/lib/rbac";

interface MemberManagementProps {
  viewerRole: Role;
}

const statusLabels: Record<MemberStatus, string> = {
  active: "正常",
  restricted: "限制中",
  suspended: "停權",
};

const riskLabels: Record<RiskLevel, string> = {
  low: "低風險",
  medium: "中風險",
  high: "高風險",
};

const actionLabels: Record<MemberActionType, string> = {
  add_note: "新增處理註記",
  request_kyc_review: "提交 KYC 覆核",
  restrict_withdrawals: "提交提款限制",
  escalate_case: "升級人工審批",
};

export function MemberManagement({ viewerRole }: MemberManagementProps) {
  const [lookupUserId, setLookupUserId] = useState("MEM-001");
  const [summary, setSummary] = useState<MemberManagementSummary | null>(null);
  const [lookupError, setLookupError] = useState<string | null>(null);
  const [lookupLoading, setLookupLoading] = useState(false);

  const [actionType, setActionType] =
    useState<MemberActionType>("add_note");
  const [ticketId, setTicketId] = useState("OPS-");
  const [reason, setReason] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionResult, setActionResult] =
    useState<MemberActionResponse | null>(null);
  const [actionLoading, setActionLoading] = useState(false);

  const canSubmit = viewerRole === "admin" || viewerRole === "manager";
  const selectedUserId = summary?.member.user_id ?? lookupUserId.trim();
  const correlationPreview = useMemo(
    () => `admin-member:${actionType}:<uuid-v4>`,
    [actionType],
  );

  async function lookupMember(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const userId = lookupUserId.trim();
    if (!userId) {
      setLookupError("請輸入會員 ID。");
      return;
    }

    setLookupLoading(true);
    setLookupError(null);
    setActionError(null);
    setActionResult(null);
    try {
      const response = await fetch(
        `/api/members/${encodeURIComponent(userId)}/summary`,
        { cache: "no-store" },
      );
      if (!response.ok) {
        setSummary(null);
        setLookupError(`查詢失敗：${await errorText(response)}`);
        return;
      }
      const nextSummary = (await response.json()) as MemberManagementSummary;
      setSummary(nextSummary);
      setLookupUserId(nextSummary.member.user_id);
    } catch {
      setSummary(null);
      setLookupError("查詢失敗：無法連線至會員管理 BFF。");
    } finally {
      setLookupLoading(false);
    }
  }

  async function submitAction(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setActionError(null);
    setActionResult(null);

    const targetUserId = selectedUserId.trim();
    if (!targetUserId) {
      setActionError("請先查詢或輸入目標會員 ID。");
      return;
    }
    if (!ticketId.trim() || !reason.trim()) {
      setActionError("請填寫工單 ID 與處理原因。");
      return;
    }

    setActionLoading(true);
    try {
      const response = await fetch("/api/members/actions", {
        method: "POST",
        cache: "no-store",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          target_user_id: targetUserId,
          action_type: actionType,
          ticket_id: ticketId.trim(),
          reason: reason.trim(),
        }),
      });
      if (!response.ok) {
        setActionError(`提交失敗：${await errorText(response)}`);
        return;
      }
      setActionResult((await response.json()) as MemberActionResponse);
    } catch {
      setActionError("提交失敗：無法連線至會員管理 BFF。");
    } finally {
      setActionLoading(false);
    }
  }

  return (
    <div className="space-y-6">
      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              會員查詢
            </h2>
            <p className="mt-2 max-w-2xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              以會員 ID 查詢聯絡資料遮罩、帳戶點數、風險狀態、限制與待處理案件。
            </p>
          </div>
          <div className="flex flex-wrap gap-2" aria-label="可查詢範例">
            {memberSearchHints.map((hint) => (
              <button
                key={hint}
                type="button"
                onClick={() => setLookupUserId(hint)}
                className="rounded-proph-pill border border-proph-divider-soft bg-proph-surface-pearl px-3 py-2 text-[12px] font-semibold text-proph-ink-muted-80 hover:border-proph-primary-focus hover:text-proph-ink"
              >
                {hint}
              </button>
            ))}
          </div>
        </div>

        <form className="mt-5 flex flex-col gap-3 sm:flex-row" onSubmit={lookupMember}>
          <label className="min-w-0 flex-1">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              會員 ID
            </span>
            <input
              value={lookupUserId}
              onChange={(event) => setLookupUserId(event.target.value)}
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus"
              placeholder="MEM-001"
            />
          </label>
          <button
            type="submit"
            disabled={lookupLoading}
            className="mt-auto inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {lookupLoading ? "查詢中" : "查詢會員"}
          </button>
        </form>

        {lookupError && <Alert tone="error">{lookupError}</Alert>}
      </section>

      {summary ? (
        <div className="grid gap-6 xl:grid-cols-[1.1fr_0.9fr]">
          <section className="space-y-6">
            <MemberOverview summary={summary} />
            <OpenCases cases={summary.open_cases} />
          </section>
          <section className="space-y-6">
            <ActionPanel
              actionType={actionType}
              actionLoading={actionLoading}
              actionResult={actionResult}
              canSubmit={canSubmit}
              correlationPreview={correlationPreview}
              error={actionError}
              reason={reason}
              selectedUserId={selectedUserId}
              ticketId={ticketId}
              onActionTypeChange={setActionType}
              onReasonChange={setReason}
              onSubmit={submitAction}
              onTicketIdChange={setTicketId}
            />
            <AuditPanel summary={summary} />
          </section>
        </div>
      ) : (
        <section className="rounded-proph-lg border border-dashed border-proph-divider-soft bg-proph-surface-pearl p-6 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          輸入會員 ID 後即可檢視聯絡、帳戶、狀態與提交處理動作。
        </section>
      )}
    </div>
  );
}

function MemberOverview({ summary }: { summary: MemberManagementSummary }) {
  const statusTone = summary.member.status === "active" ? "success" : "warning";
  const riskTone = summary.kyc_risk.risk_level === "high" ? "critical" : "neutral";

  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-wrap items-center gap-3">
        <StatusPill tone={statusTone}>{statusLabels[summary.member.status]}</StatusPill>
        <StatusPill tone={riskTone}>{riskLabels[summary.kyc_risk.risk_level]}</StatusPill>
        <span className="font-mono text-[12px] text-proph-ink-muted-48">
          {summary.member.user_id}
        </span>
      </div>
      <h2 className="mt-4 text-[24px] font-semibold leading-[1.15] tracking-[-0.224px]">
        {summary.member.display_name}
      </h2>
      <p className="mt-1 text-[14px] text-proph-ink-muted-80">
        {summary.member.tier} / 註冊於 {formatDateTime(summary.member.registered_at)}
      </p>

      <div className="mt-6 grid gap-4 lg:grid-cols-2">
        <InfoCard title="聯絡資料">
          <Metric label="Email" value={summary.member.contact.email_masked} />
          <Metric label="手機" value={summary.member.contact.phone_masked} />
          <Metric label="LINE" value={summary.member.contact.line_id_masked ?? "未綁定"} />
          <Metric label="地區" value={`${summary.member.contact.country} / ${summary.member.contact.timezone}`} />
        </InfoCard>
        <InfoCard title="帳戶摘要">
          <Metric label="可用點數" value={formatMinorUnitsAsMemberPoints(summary.account.available_minor_units)} />
          <Metric label="鎖定點數" value={formatMinorUnitsAsMemberPoints(summary.account.locked_minor_units)} />
          <Metric label="凍結點數" value={formatMinorUnitsAsMemberPoints(summary.account.frozen_minor_units)} />
          <Metric label="2FA" value={summary.account.two_factor_enabled ? "已啟用" : "未啟用"} />
        </InfoCard>
        <InfoCard title="狀態與風控">
          <Metric label="KYC" value={summary.kyc_risk.kyc_status} />
          <Metric label="風險分數" value={String(summary.kyc_risk.risk_score)} />
          <Metric label="限制" value={summary.kyc_risk.restrictions.join(", ") || "無"} />
          <Metric label="最近覆核" value={formatOptionalDateTime(summary.kyc_risk.last_reviewed_at)} />
        </InfoCard>
        <InfoCard title="登入與裝置">
          <Metric label="最近登入" value={formatOptionalDateTime(summary.account.last_login_at)} />
          <Metric label="裝置數" value={`${summary.account.device_count} 台`} />
          <Metric label="幣別" value={summary.account.currency} />
          <Metric label="會員狀態" value={statusLabels[summary.member.status]} />
        </InfoCard>
      </div>
    </section>
  );
}

function OpenCases({ cases }: { cases: MemberManagementSummary["open_cases"] }) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        待處理案件
      </h2>
      {cases.length === 0 ? (
        <p className="mt-4 text-[14px] text-proph-ink-muted-80">目前沒有未結案件。</p>
      ) : (
        <div className="mt-4 grid gap-3">
          {cases.map((item) => (
            <article
              key={item.case_id}
              className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4"
            >
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h3 className="font-mono text-[13px] font-semibold text-proph-ink">
                  {item.case_id}
                </h3>
                <StatusPill tone={item.status === "blocked" ? "warning" : "neutral"}>
                  {item.status}
                </StatusPill>
              </div>
              <p className="mt-2 text-[14px] text-proph-ink-muted-80">
                {item.case_type} / {item.owner_role} / {item.age}
              </p>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function ActionPanel(props: {
  actionType: MemberActionType;
  actionLoading: boolean;
  actionResult: MemberActionResponse | null;
  canSubmit: boolean;
  correlationPreview: string;
  error: string | null;
  reason: string;
  selectedUserId: string;
  ticketId: string;
  onActionTypeChange: (value: MemberActionType) => void;
  onReasonChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onTicketIdChange: (value: string) => void;
}) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        提交處理動作
      </h2>
      <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        提交後只建立審批與稽核紀錄；限制、補償或帳本寫入仍需後續核准流程。
      </p>
      <form className="mt-5 grid gap-4" onSubmit={props.onSubmit}>
        <Metric label="目標會員" value={props.selectedUserId || "未選擇"} />
        <label className="block">
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">動作</span>
          <select
            value={props.actionType}
            onChange={(event) => props.onActionTypeChange(event.target.value as MemberActionType)}
            disabled={!props.canSubmit || props.actionLoading}
            className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
          >
            {Object.entries(actionLabels).map(([value, label]) => (
              <option key={value} value={value}>{label}</option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">工單 ID</span>
          <input
            value={props.ticketId}
            onChange={(event) => props.onTicketIdChange(event.target.value)}
            disabled={!props.canSubmit || props.actionLoading}
            className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
            placeholder="OPS-1042"
          />
        </label>
        <label className="block">
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">處理原因</span>
          <textarea
            value={props.reason}
            onChange={(event) => props.onReasonChange(event.target.value)}
            disabled={!props.canSubmit || props.actionLoading}
            className="mt-2 min-h-28 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 py-2 text-[14px] leading-[1.43] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
            placeholder="描述客服、合規或營運判斷依據"
          />
        </label>
        <button
          type="submit"
          disabled={!props.canSubmit || props.actionLoading}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
        >
          {props.actionLoading ? "提交中" : "提交動作"}
        </button>
        <p className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {props.correlationPreview}
        </p>
      </form>
      {props.error && <Alert tone="error">{props.error}</Alert>}
      {props.actionResult && (
        <Alert tone="success">
          已提交 {props.actionResult.action_id}，狀態：{props.actionResult.status}，correlation id：{props.actionResult.audit.correlation_id}
        </Alert>
      )}
    </section>
  );
}

function AuditPanel({ summary }: { summary: MemberManagementSummary }) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        稽核姿態
      </h2>
      <div className="mt-4 grid gap-3">
        <Metric label="來源" value={summary.audit.source} />
        <Metric label="Correlation ID" value={summary.audit.correlation_id} />
        <Metric label="需稽核" value={summary.audit.audit_required ? "是" : "否"} />
        <Metric label="最近管理動作" value={formatOptionalDateTime(summary.audit.latest_admin_action_at)} />
      </div>
    </section>
  );
}

function InfoCard({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4">
      <h3 className="text-[15px] font-semibold leading-[1.29] text-proph-ink">{title}</h3>
      <dl className="mt-3 grid gap-2">{children}</dl>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-4 rounded-proph-sm border border-proph-divider-soft bg-proph-canvas px-3 py-2">
      <dt className="text-[12px] font-semibold text-proph-ink-muted-80">{label}</dt>
      <dd className="text-right font-mono text-[13px] text-proph-ink">{value}</dd>
    </div>
  );
}

function Alert({ children, tone }: { children: ReactNode; tone: "error" | "success" }) {
  const border = tone === "error" ? "border-proph-feedback-error" : "border-proph-feedback-success";
  return (
    <p className={`mt-4 rounded-proph-sm border ${border} bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink`}>
      {children}
    </p>
  );
}

function formatOptionalDateTime(value: string | null): string {
  return value ? formatDateTime(value) : "尚無紀錄";
}

function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat("zh-Hant-TW", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

async function errorText(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string };
    return body.error ?? response.statusText;
  } catch {
    return response.statusText;
  }
}
