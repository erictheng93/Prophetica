"use client";

import {
  useEffect,
  useMemo,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import type {
  CompensationPostingRetryResponse,
  CompensationProposal,
  CompensationProposalCreateResponse,
  CompensationProposalListResponse,
  CompensationProposalStatus,
  CompensationReviewDecision,
  CompensationReviewResponse,
} from "@/lib/compensation";
import type { Role } from "@/lib/rbac";

interface CompensationWorkspaceProps {
  viewerRole: Role;
  viewerUserId: string;
}

type QueueKey = "proposed" | "stuck" | "history";

interface CompensationFormState {
  subjectUserId: string;
  ticketId: string;
  reason: string;
  amount: string;
}

const emptyForm: CompensationFormState = {
  subjectUserId: "",
  ticketId: "OPS-",
  reason: "",
  amount: "",
};

const statusLabels: Record<CompensationProposalStatus, string> = {
  proposed: "待審",
  approved: "已核准",
  posting: "入帳中",
  posted: "已入帳",
  posting_failed: "Ledger 拒絕",
  posting_unknown: "結果待確認（可能已入帳）",
  rejected: "已駁回",
  cancelled: "已取消",
};

const statusTone: Record<CompensationProposalStatus, string> = {
  proposed: "border-proph-primary bg-proph-surface-pearl text-proph-primary",
  approved: "border-proph-divider-soft bg-proph-surface-pearl text-proph-ink-muted-80",
  posting: "border-proph-divider-soft bg-proph-surface-pearl text-proph-ink-muted-80",
  posted: "border-proph-feedback-success bg-proph-surface-pearl text-proph-ink",
  posting_failed: "border-proph-feedback-error bg-proph-surface-pearl text-proph-ink",
  posting_unknown: "border-proph-feedback-warning bg-proph-surface-pearl text-proph-ink",
  rejected: "border-proph-divider-soft bg-proph-surface-pearl text-proph-ink-muted-48",
  cancelled: "border-proph-divider-soft bg-proph-surface-pearl text-proph-ink-muted-48",
};

const stuckStatuses: CompensationProposalStatus[] = [
  "posting",
  "posting_failed",
  "posting_unknown",
];

const historyStatuses: CompensationProposalStatus[] = ["posted", "rejected"];

export function CompensationWorkspace({
  viewerRole,
  viewerUserId,
}: CompensationWorkspaceProps) {
  const [form, setForm] = useState<CompensationFormState>(emptyForm);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitResult, setSubmitResult] =
    useState<CompensationProposalCreateResponse | null>(null);

  const [proposed, setProposed] = useState<CompensationProposal[]>([]);
  const [stuck, setStuck] = useState<CompensationProposal[]>([]);
  const [history, setHistory] = useState<CompensationProposal[]>([]);
  const [queueLoading, setQueueLoading] = useState<Record<QueueKey, boolean>>({
    proposed: false,
    stuck: false,
    history: false,
  });
  const [queueErrors, setQueueErrors] = useState<Record<QueueKey, string | null>>({
    proposed: null,
    stuck: null,
    history: null,
  });
  const [reviewLoading, setReviewLoading] = useState<string | null>(null);
  const [retryLoading, setRetryLoading] = useState<string | null>(null);
  const [mutationMessage, setMutationMessage] = useState<string | null>(null);

  const canRetry = viewerRole === "admin";
  const canUseWorkspace = viewerRole === "admin" || viewerRole === "manager";
  const amountMinorUnits = useMemo(() => parseAmountToMinorUnits(form.amount), [form.amount]);

  useEffect(() => {
    void refreshAllQueues();
  }, []);

  async function refreshAllQueues() {
    await Promise.all([refreshProposed(), refreshStuck(), refreshHistory()]);
  }

  async function refreshProposed() {
    await loadQueue("proposed", ["proposed"], setProposed);
  }

  async function refreshStuck() {
    await loadQueue("stuck", stuckStatuses, setStuck);
  }

  async function refreshHistory() {
    await loadQueue("history", historyStatuses, setHistory);
  }

  async function loadQueue(
    key: QueueKey,
    statuses: CompensationProposalStatus[],
    setRows: (rows: CompensationProposal[]) => void,
  ) {
    setQueueLoading((current) => ({ ...current, [key]: true }));
    setQueueErrors((current) => ({ ...current, [key]: null }));
    try {
      const rows = await Promise.all(statuses.map((status) => fetchProposals(status)));
      setRows(rows.flat().filter((proposal) => proposal.status !== "cancelled"));
    } catch (error) {
      setRows([]);
      setQueueErrors((current) => ({
        ...current,
        [key]: error instanceof Error ? error.message : "載入失敗。",
      }));
    } finally {
      setQueueLoading((current) => ({ ...current, [key]: false }));
    }
  }

  async function fetchProposals(status: CompensationProposalStatus) {
    const params = new URLSearchParams({ status });
    const response = await fetch(`/api/compensation?${params}`, {
      cache: "no-store",
    });
    if (!response.ok) {
      throw new Error(await compensationErrorMessage(response));
    }
    const body = (await response.json()) as CompensationProposalListResponse;
    return body.proposals;
  }

  async function submitProposal(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitError(null);
    setSubmitResult(null);
    setMutationMessage(null);

    const subjectUserId = form.subjectUserId.trim();
    const ticketId = form.ticketId.trim();
    const reason = form.reason.trim();
    if (!subjectUserId) {
      setSubmitError("請輸入 subject_user_id。");
      return;
    }
    if (!ticketId || !reason) {
      setSubmitError("請填寫 ticket 與 reason。");
      return;
    }
    if (amountMinorUnits === null) {
      setSubmitError("補償金額格式不正確，請輸入正數且最多兩位小數。");
      return;
    }

    setSubmitLoading(true);
    try {
      const response = await fetch("/api/compensation", {
        method: "POST",
        cache: "no-store",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          user_id: subjectUserId,
          ticket_id: ticketId,
          reason,
          amount_minor_units: amountMinorUnits,
        }),
      });
      if (!response.ok) {
        setSubmitError(await compensationErrorMessage(response));
        return;
      }
      const body = (await response.json()) as CompensationProposalCreateResponse;
      setSubmitResult(body);
      setMutationMessage(`已建立補償提案 ${body.proposal.proposal_id}，進入待審佇列。`);
      setForm(emptyForm);
      await refreshProposed();
    } catch {
      setSubmitError("提交失敗：無法連線至補償 BFF。");
    } finally {
      setSubmitLoading(false);
    }
  }

  async function reviewProposal(
    proposal: CompensationProposal,
    decision: CompensationReviewDecision,
  ) {
    setMutationMessage(null);
    setReviewLoading(`${proposal.proposal_id}:${decision}`);
    try {
      const response = await fetch(
        `/api/compensation/${encodeURIComponent(proposal.proposal_id)}/review`,
        {
          method: "POST",
          cache: "no-store",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ decision }),
        },
      );
      if (!response.ok) {
        setMutationMessage(await compensationErrorMessage(response));
        return;
      }
      const body = (await response.json()) as CompensationReviewResponse;
      setMutationMessage(
        decision === "approved"
          ? `已核准 ${body.proposal.proposal_id}，目前狀態：${statusLabels[body.proposal.status]}。`
          : `已駁回 ${body.proposal.proposal_id}。`,
      );
      await refreshAllQueues();
    } catch {
      setMutationMessage("審核失敗：無法連線至補償 BFF。");
    } finally {
      setReviewLoading(null);
    }
  }

  async function retryPosting(proposal: CompensationProposal) {
    setMutationMessage(null);
    setRetryLoading(proposal.proposal_id);
    try {
      const response = await fetch(
        `/api/compensation/${encodeURIComponent(proposal.proposal_id)}/retry-posting`,
        {
          method: "POST",
          cache: "no-store",
        },
      );
      if (!response.ok) {
        setMutationMessage(await compensationErrorMessage(response));
        return;
      }
      const body = (await response.json()) as CompensationPostingRetryResponse;
      setMutationMessage(
        body.replayed
          ? `已 reconcile ${body.proposal.proposal_id}，未重複入帳。`
          : `已重試 ${body.proposal.proposal_id}，目前狀態：${statusLabels[body.proposal.status]}。`,
      );
      await refreshAllQueues();
    } catch {
      setMutationMessage("重試失敗：無法連線至補償 BFF。");
    } finally {
      setRetryLoading(null);
    }
  }

  if (!canUseWorkspace) {
    return (
      <div className="rounded-proph-lg border border-proph-feedback-error bg-proph-canvas p-6 text-[14px] text-proph-ink">
        此工作區僅限 admin / manager。
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {mutationMessage && (
        <div
          role="status"
          className="rounded-proph-sm border border-proph-primary bg-proph-surface-pearl p-4 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink"
        >
          {mutationMessage}
        </div>
      )}

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              補償提案
            </h2>
            <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              建立客服補償提案；核准、入帳與 retry 仍由後端狀態機守門。
            </p>
          </div>
          <StatusBadge status="proposed" />
        </div>
        <form className="mt-5 grid gap-4 md:grid-cols-2" onSubmit={submitProposal}>
          <TextField
            label="subject_user_id"
            value={form.subjectUserId}
            onChange={(value) => setForm((current) => ({ ...current, subjectUserId: value }))}
            disabled={submitLoading}
            placeholder="user_123"
          />
          <TextField
            label="ticket"
            value={form.ticketId}
            onChange={(value) => setForm((current) => ({ ...current, ticketId: value }))}
            disabled={submitLoading}
            placeholder="OPS-1042"
          />
          <TextField
            label="amount"
            value={form.amount}
            onChange={(value) => setForm((current) => ({ ...current, amount: value }))}
            disabled={submitLoading}
            inputMode="decimal"
            placeholder="100.00"
          />
          <label className="block md:col-span-2">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              reason
            </span>
            <textarea
              value={form.reason}
              onChange={(event) =>
                setForm((current) => ({ ...current, reason: event.target.value }))
              }
              disabled={submitLoading}
              className="mt-2 min-h-24 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 py-2 text-[14px] leading-[1.43] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
              placeholder="補償原因、客服處理紀錄與依據"
            />
          </label>
          <div className="md:col-span-2">
            <button
              type="submit"
              disabled={submitLoading}
              className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
            >
              {submitLoading ? "提交中" : "建立提案"}
            </button>
          </div>
        </form>
        {submitError && <ErrorNotice message={submitError} />}
        {submitResult && (
          <p className="mt-4 font-mono text-[12px] text-proph-ink-muted-80">
            audit: {submitResult.audit.action} / {submitResult.audit.proposer_kind}
          </p>
        )}
      </section>

      <section className="grid gap-6 xl:grid-cols-[1fr_1fr]">
        <QueuePanel
          title="待審佇列"
          description="proposed 狀態；提案者本人只能看到提示，實際守門由後端執行。"
          loading={queueLoading.proposed}
          error={queueErrors.proposed}
          empty="目前沒有待審補償提案。"
          itemCount={proposed.length}
        >
          {proposed.map((proposal) => {
            const ownProposal = proposal.proposer_user_id === viewerUserId;
            return (
              <ProposalCard key={proposal.proposal_id} proposal={proposal}>
                {ownProposal && (
                  <p className="rounded-proph-sm border border-proph-feedback-warning bg-proph-surface-pearl p-3 text-[13px] text-proph-ink">
                    不可審自己的提案
                  </p>
                )}
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    disabled={ownProposal || reviewLoading !== null}
                    onClick={() => void reviewProposal(proposal, "approved")}
                    className="inline-flex h-10 items-center rounded-proph-pill bg-proph-primary px-4 text-[13px] font-semibold text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {reviewLoading === `${proposal.proposal_id}:approved` ? "核准中" : "approve"}
                  </button>
                  <button
                    type="button"
                    disabled={ownProposal || reviewLoading !== null}
                    onClick={() => void reviewProposal(proposal, "rejected")}
                    className="inline-flex h-10 items-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-4 text-[13px] font-semibold text-proph-ink disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {reviewLoading === `${proposal.proposal_id}:rejected` ? "駁回中" : "reject"}
                  </button>
                </div>
              </ProposalCard>
            );
          })}
        </QueuePanel>

        <QueuePanel
          title="卡住佇列"
          description="posting / posting_failed / posting_unknown；admin 可手動 retry，後端先查既有 idempotency key。"
          loading={queueLoading.stuck}
          error={queueErrors.stuck}
          empty="目前沒有卡住的入帳案件。"
          itemCount={stuck.length}
        >
          {stuck.map((proposal) => (
            <ProposalCard key={proposal.proposal_id} proposal={proposal} showPostingDetails>
              {canRetry ? (
                <button
                  type="button"
                  disabled={retryLoading !== null}
                  onClick={() => void retryPosting(proposal)}
                  className="inline-flex h-10 items-center rounded-proph-pill bg-proph-ink px-4 text-[13px] font-semibold text-proph-on-dark disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {retryLoading === proposal.proposal_id ? "retry 中" : "retry posting"}
                </button>
              ) : (
                <p className="text-[13px] text-proph-ink-muted-80">
                  manager 可檢視；manual retry 僅限 admin。
                </p>
              )}
            </ProposalCard>
          ))}
        </QueuePanel>
      </section>

      <QueuePanel
        title="歷史區"
        description="posted / rejected 唯讀清單；v1 不顯示 cancelled。"
        loading={queueLoading.history}
        error={queueErrors.history}
        empty="目前沒有補償歷史紀錄。"
        itemCount={history.length}
      >
        <div className="grid gap-3 lg:grid-cols-2">
          {history.map((proposal) => (
            <ProposalCard key={proposal.proposal_id} proposal={proposal} compact />
          ))}
        </div>
      </QueuePanel>
    </div>
  );
}

function QueuePanel({
  title,
  description,
  loading,
  error,
  empty,
  itemCount,
  children,
}: {
  title: string;
  description: string;
  loading: boolean;
  error: string | null;
  empty: string;
  itemCount: number;
  children: ReactNode;
}) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {title}
      </h2>
      <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {description}
      </p>
      {loading && <p className="mt-4 text-[14px] text-proph-ink-muted-80">載入中...</p>}
      {error && <ErrorNotice message={error} />}
      {!loading && !error && itemCount === 0 ? (
        <p className="mt-4 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4 text-[14px] text-proph-ink-muted-80">
          {empty}
        </p>
      ) : null}
      {!loading && !error && itemCount > 0 ? (
        <div className="mt-5 space-y-3">{children}</div>
      ) : null}
    </section>
  );
}

function ProposalCard({
  proposal,
  children,
  compact = false,
  showPostingDetails = false,
}: {
  proposal: CompensationProposal;
  children?: ReactNode;
  compact?: boolean;
  showPostingDetails?: boolean;
}) {
  return (
    <article className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="font-mono text-[14px] font-semibold text-proph-ink">
            {proposal.proposal_id}
          </h3>
          <p className="mt-1 text-[13px] text-proph-ink-muted-80">
            {proposal.user_id} / {proposal.ticket_id}
          </p>
        </div>
        <StatusBadge status={proposal.status} />
      </div>
      <dl className="mt-4 grid gap-2 text-[13px] leading-[1.43] text-proph-ink-muted-80">
        <MetaRow label="amount" value={formatMinorUnits(proposal.amount_minor_units)} />
        {!compact && <MetaRow label="reason" value={proposal.reason} />}
        <MetaRow label="proposer" value={proposal.proposer_kind} />
        <MetaRow label="attempts" value={String(proposal.posting_attempt_count)} />
        {showPostingDetails && (
          <>
            <MetaRow label="approved_at" value={proposal.approved_at ?? "—"} />
            <MetaRow label="last_error" value={proposal.last_posting_error ?? "—"} />
            <MetaRow label="ledger_key" value={proposal.ledger_idempotency_key ?? "—"} />
          </>
        )}
      </dl>
      {children ? <div className="mt-4 space-y-3">{children}</div> : null}
    </article>
  );
}

function StatusBadge({ status }: { status: CompensationProposalStatus }) {
  return (
    <span
      className={`inline-flex rounded-proph-pill border px-3 py-1 text-[12px] font-semibold leading-none ${statusTone[status]}`}
    >
      {statusLabels[status]}
    </span>
  );
}

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[7rem_1fr] gap-3">
      <dt className="font-semibold text-proph-ink-muted-48">{label}</dt>
      <dd className="break-words text-proph-ink-muted-80">{value}</dd>
    </div>
  );
}

function TextField({
  label,
  value,
  onChange,
  disabled,
  placeholder,
  inputMode,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
  inputMode?: "decimal";
}) {
  return (
    <label className="block">
      <span className="text-[13px] font-semibold text-proph-ink-muted-80">
        {label}
      </span>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        disabled={disabled}
        inputMode={inputMode}
        className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
        placeholder={placeholder}
      />
    </label>
  );
}

function ErrorNotice({ message }: { message: string }) {
  return (
    <p className="mt-4 rounded-proph-sm border border-proph-feedback-error bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
      {message}
    </p>
  );
}

function parseAmountToMinorUnits(input: string): number | null {
  const trimmed = input.trim();
  const match = /^(\d+)(?:\.(\d{1,2}))?$/.exec(trimmed);
  if (!match) return null;
  const major = BigInt(match[1] ?? "0");
  const minorText = (match[2] ?? "").padEnd(2, "0");
  const minor = major * 100n + BigInt(minorText === "" ? "0" : minorText);
  if (minor <= 0n || minor > BigInt(Number.MAX_SAFE_INTEGER)) return null;
  return Number(minor);
}

function formatMinorUnits(value: number): string {
  const major = Math.floor(Math.abs(value) / 100);
  const minor = String(Math.abs(value) % 100).padStart(2, "0");
  return `${major.toLocaleString("zh-Hant")}.${minor}`;
}

async function compensationErrorMessage(response: Response): Promise<string> {
  const prefix = responseStatusPrefix(response.status);
  try {
    const payload = (await response.json()) as {
      error?: string;
      message?: string;
    };
    return `${prefix}${payload.message ?? payload.error ?? response.status}`;
  } catch {
    return `${prefix}${response.status}`;
  }
}

function responseStatusPrefix(status: number): string {
  if (status === 404) return "找不到案件或會員：";
  if (status === 422) return "資料驗證失敗：";
  if (status === 502 || status === 503) return "後端服務暫不可用：";
  if (status >= 500) return "系統錯誤：";
  return "請求失敗：";
}
