"use client";

import {
  useEffect,
  useMemo,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import Link from "next/link";
import { KycDocumentGallery } from "@/components/kyc-document-gallery";
import { StatusPill } from "@/components/status-pill";
import {
  formatMinorUnitsAsMemberPoints,
  memberSearchHints,
  type MemberActionListResponse,
  type MemberActionRecord,
  type MemberActionResponse,
  type MemberActionStatus,
  type MemberActionType,
  type MemberManagementSummary,
  type MemberPendingCasesResponse,
  type MemberStatus,
  type RiskLevel,
} from "@/lib/member-management-shared";
import type { Role } from "@/lib/rbac";

interface MemberManagementProps {
  initialQuery?: string;
  initialPendingRestriction?: string;
  initialPendingStatus?: MemberStatus | "all";
  showActionQueue?: boolean;
  showMemberWorkspace?: boolean;
  showPendingCases?: boolean;
  viewerRole: Role;
  viewerAffiliateId?: string;
}

const statusLabels: Record<MemberStatus, string> = {
  inactive: "未激活",
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
  escalate_case: "升級工單審批",
};

interface MemberFormState {
  user_id: string;
  display_name: string;
  email: string;
  phone: string;
  line_id: string;
  country: string;
  timezone: string;
  tier: string;
  affiliate_id: string;
  kyc_status: "pending" | "review_required" | "verified";
  risk_level: "low" | "medium" | "high";
  risk_score: string;
}

const emptyMemberForm: MemberFormState = {
  user_id: "",
  display_name: "",
  email: "",
  phone: "",
  line_id: "",
  country: "TW",
  timezone: "Asia/Taipei",
  tier: "Standard",
  affiliate_id: "",
  kyc_status: "pending",
  risk_level: "medium",
  risk_score: "50",
};

export function MemberManagement({
  initialQuery,
  initialPendingRestriction,
  initialPendingStatus,
  showActionQueue = false,
  showMemberWorkspace = true,
  showPendingCases = false,
  viewerAffiliateId,
  viewerRole,
}: MemberManagementProps) {
  const [lookupUserId, setLookupUserId] = useState(initialQuery ?? "");
  const [members, setMembers] = useState<MemberManagementSummary[]>([]);
  const [memberListError, setMemberListError] = useState<string | null>(null);
  const [memberListLoading, setMemberListLoading] = useState(false);
  const [summary, setSummary] = useState<MemberManagementSummary | null>(null);
  const [lookupError, setLookupError] = useState<string | null>(null);
  const [lookupLoading, setLookupLoading] = useState(false);
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
  const [memberForm, setMemberForm] = useState<MemberFormState>({
    ...emptyMemberForm,
    affiliate_id: viewerRole === "affiliate" ? (viewerAffiliateId ?? "") : "",
  });
  const [editingUserId, setEditingUserId] = useState<string | null>(null);
  const [memberMutationError, setMemberMutationError] = useState<string | null>(
    null,
  );
  const [memberMutationResult, setMemberMutationResult] = useState<
    string | null
  >(null);
  const [memberMutationLoading, setMemberMutationLoading] = useState(false);
  const [memberEditorOpen, setMemberEditorOpen] = useState(false);
  const [pendingCases, setPendingCases] =
    useState<MemberPendingCasesResponse | null>(null);
  const [pendingCasesError, setPendingCasesError] = useState<string | null>(
    null,
  );
  const [pendingCasesLoading, setPendingCasesLoading] = useState(false);
  const [pendingQuery, setPendingQuery] = useState("");
  const [pendingStatus, setPendingStatus] = useState<MemberStatus | "all">(
    initialPendingStatus ?? "all",
  );
  const [pendingRestriction, setPendingRestriction] = useState(
    initialPendingRestriction ?? "",
  );
  const [pendingPage, setPendingPage] = useState(1);

  const [actionType, setActionType] = useState<MemberActionType>("add_note");
  const [ticketId, setTicketId] = useState("OPS-");
  const [reason, setReason] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionResult, setActionResult] = useState<MemberActionResponse | null>(
    null,
  );
  const [actionLoading, setActionLoading] = useState(false);
  const [actionQueue, setActionQueue] =
    useState<MemberActionListResponse | null>(null);
  const [actionQueueError, setActionQueueError] = useState<string | null>(null);
  const [actionQueueLoading, setActionQueueLoading] = useState(false);
  const [actionReviewReasons, setActionReviewReasons] = useState<
    Record<string, string>
  >({});
  const [actionReviewLoading, setActionReviewLoading] = useState<string | null>(
    null,
  );

  const canDelete = viewerRole === "admin" || viewerRole === "manager";
  const canSubmit = viewerRole === "admin" || viewerRole === "manager";
  const selectedUserId = summary?.member.user_id ?? lookupUserId.trim();
  const correlationPreview = useMemo(
    () => `admin-member:${actionType}:<uuid-v4>`,
    [actionType],
  );
  const memberSuggestions = useMemo(() => {
    const query = lookupUserId.trim().toLowerCase();
    if (query.length < 2) return [];
    return members
      .filter(
        (entry) =>
          entry.member.user_id.toLowerCase().includes(query) ||
          entry.member.display_name.toLowerCase().includes(query),
      )
      .slice(0, 8);
  }, [lookupUserId, members]);

  useEffect(() => {
    if (!showMemberWorkspace) return;
    void refreshMembers();
    if (initialQuery) void lookupMemberById(initialQuery);
  }, [showMemberWorkspace]);

  useEffect(() => {
    if (canSubmit && showActionQueue) void refreshActionQueue();
  }, [canSubmit, showActionQueue]);

  useEffect(() => {
    if (!showPendingCases) return;
    void refreshPendingCases();
  }, [pendingPage, pendingStatus, pendingRestriction, showPendingCases]);

  async function refreshPendingCases(nextPage = pendingPage) {
    setPendingCasesLoading(true);
    setPendingCasesError(null);
    const params = new URLSearchParams({
      page: String(nextPage),
      page_size: "5",
    });
    if (pendingQuery.trim()) params.set("q", pendingQuery.trim());
    if (pendingStatus !== "all") params.set("status", pendingStatus);
    if (pendingRestriction.trim()) {
      params.set("restriction", pendingRestriction.trim());
    }
    try {
      const response = await fetch(`/api/members/pending-cases?${params}`, {
        cache: "no-store",
      });
      if (!response.ok) {
        setPendingCases(null);
        setPendingCasesError(
          `載入待處理會員失敗：${await errorText(response)}`,
        );
        return;
      }
      setPendingCases((await response.json()) as MemberPendingCasesResponse);
    } catch {
      setPendingCases(null);
      setPendingCasesError("載入待處理會員失敗：無法連線至會員待處理 API。");
    } finally {
      setPendingCasesLoading(false);
    }
  }

  async function refreshMembers() {
    setMemberListLoading(true);
    setMemberListError(null);
    try {
      const response = await fetch("/api/members", { cache: "no-store" });
      if (!response.ok) {
        setMembers([]);
        setMemberListError(`載入會員清單失敗：${await errorText(response)}`);
        return;
      }
      const body = (await response.json()) as {
        members: MemberManagementSummary[];
      };
      setMembers(body.members);
    } catch {
      setMembers([]);
      setMemberListError("載入會員清單失敗：無法連線至會員管理 BFF。");
    } finally {
      setMemberListLoading(false);
    }
  }

  async function lookupMember(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSuggestionsOpen(false);
    const userId = lookupUserId.trim();
    if (!userId) {
      setLookupError("請輸入會員 ID。");
      return;
    }

    await lookupMemberById(userId);
  }

  async function lookupMemberById(userId: string) {
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
        setLookupError(
          response.status === 404
            ? "查無此會員，請確認輸入的會員 ID 是否正確。"
            : "查詢服務暫時無法使用，請稍後再試。",
        );
        return;
      }
      const nextSummary = (await response.json()) as MemberManagementSummary;
      setSummary(nextSummary);
      setLookupUserId(nextSummary.member.user_id);
      startEditing(nextSummary);
    } catch {
      setSummary(null);
      setLookupError("查詢服務暫時無法使用，請稍後再試。");
    } finally {
      setLookupLoading(false);
    }
  }

  function selectSuggestion(userId: string) {
    setSuggestionsOpen(false);
    setLookupUserId(userId);
    void lookupMemberById(userId);
  }

  function updateMemberForm<K extends keyof MemberFormState>(
    key: K,
    value: MemberFormState[K],
  ) {
    setMemberForm((current) => ({ ...current, [key]: value }));
  }

  function resetMemberForm() {
    setEditingUserId(null);
    setMemberMutationError(null);
    setMemberMutationResult(null);
    setMemberForm({
      ...emptyMemberForm,
      affiliate_id: viewerRole === "affiliate" ? (viewerAffiliateId ?? "") : "",
    });
  }

  function openCreateMemberEditor() {
    resetMemberForm();
    setMemberEditorOpen(true);
  }

  function selectMember(nextSummary: MemberManagementSummary) {
    setSummary(nextSummary);
    setLookupUserId(nextSummary.member.user_id);
    setLookupError(null);
    startEditing(nextSummary);
  }

  function openEditMemberEditor(nextSummary: MemberManagementSummary) {
    selectMember(nextSummary);
    setMemberEditorOpen(true);
  }

  function startEditing(nextSummary: MemberManagementSummary) {
    setEditingUserId(nextSummary.member.user_id);
    setMemberMutationError(null);
    setMemberMutationResult(null);
    setMemberForm({
      user_id: nextSummary.member.user_id,
      display_name: nextSummary.member.display_name,
      email: "",
      phone: "",
      line_id: "",
      country: nextSummary.member.contact.country,
      timezone: nextSummary.member.contact.timezone,
      tier: nextSummary.member.tier,
      affiliate_id: nextSummary.source.affiliate_id ?? "",
      kyc_status: nextSummary.kyc_risk.kyc_status,
      risk_level: nextSummary.kyc_risk.risk_level,
      risk_score: String(nextSummary.kyc_risk.risk_score),
    });
  }

  async function submitMemberMutation(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMemberMutationError(null);
    setMemberMutationResult(null);
    setMemberMutationLoading(true);

    const payload = {
      user_id: memberForm.user_id.trim(),
      display_name: memberForm.display_name.trim(),
      email: memberForm.email.trim(),
      phone: memberForm.phone.trim(),
      line_id: memberForm.line_id.trim() || undefined,
      country: memberForm.country.trim(),
      timezone: memberForm.timezone.trim(),
      tier: memberForm.tier.trim(),
      affiliate_id: memberForm.affiliate_id.trim() || undefined,
      kyc_status: memberForm.kyc_status,
      risk_level: memberForm.risk_level,
      risk_score: Number(memberForm.risk_score),
    };

    try {
      const response = await fetch(
        editingUserId
          ? `/api/members/${encodeURIComponent(editingUserId)}`
          : "/api/members",
        {
          method: editingUserId ? "PATCH" : "POST",
          cache: "no-store",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(payload),
        },
      );
      if (!response.ok) {
        setMemberMutationError(`儲存失敗：${await errorText(response)}`);
        return;
      }
      const nextSummary = (await response.json()) as MemberManagementSummary;
      setSummary(nextSummary);
      setLookupUserId(nextSummary.member.user_id);
      startEditing(nextSummary);
      setMemberMutationResult(
        editingUserId ? "會員資料已更新。" : "會員已新增，帳戶狀態為未激活。",
      );
      setMemberEditorOpen(false);
      await refreshMembers();
    } catch {
      setMemberMutationError("儲存失敗：無法連線至會員管理 BFF。");
    } finally {
      setMemberMutationLoading(false);
    }
  }

  async function deleteSelectedMember() {
    if (!summary || !canDelete) return;
    setMemberMutationError(null);
    setMemberMutationResult(null);
    setMemberMutationLoading(true);
    try {
      const response = await fetch(
        `/api/members/${encodeURIComponent(summary.member.user_id)}`,
        { method: "DELETE", cache: "no-store" },
      );
      if (!response.ok) {
        setMemberMutationError(`刪除失敗：${await errorText(response)}`);
        return;
      }
      setMemberMutationResult(`已刪除會員 ${summary.member.user_id}。`);
      setMemberEditorOpen(false);
      setSummary(null);
      resetMemberForm();
      await refreshMembers();
    } catch {
      setMemberMutationError("刪除失敗：無法連線至會員管理 BFF。");
    } finally {
      setMemberMutationLoading(false);
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
      await refreshActionQueue();
    } catch {
      setActionError("提交失敗：無法連線至會員管理 BFF。");
    } finally {
      setActionLoading(false);
    }
  }

  async function refreshActionQueue() {
    setActionQueueLoading(true);
    setActionQueueError(null);
    try {
      const response = await fetch("/api/members/actions", {
        cache: "no-store",
      });
      if (!response.ok) {
        setActionQueue(null);
        setActionQueueError(`載入工單失敗：${await errorText(response)}`);
        return;
      }
      setActionQueue((await response.json()) as MemberActionListResponse);
    } catch {
      setActionQueue(null);
      setActionQueueError("載入工單失敗：無法連線至會員管理 BFF。");
    } finally {
      setActionQueueLoading(false);
    }
  }

  async function reviewAction(
    actionId: string,
    decision: "approve" | "reject",
  ) {
    const reason = actionReviewReasons[actionId]?.trim();
    if (!reason) {
      setActionQueueError("請先填寫審批原因。");
      return;
    }
    setActionReviewLoading(`${actionId}:${decision}`);
    setActionQueueError(null);
    try {
      const response = await fetch(
        `/api/members/actions/${encodeURIComponent(actionId)}/reviews`,
        {
          method: "POST",
          cache: "no-store",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ decision, reason }),
        },
      );
      if (!response.ok) {
        setActionQueueError(`審批失敗：${await errorText(response)}`);
        return;
      }
      await refreshActionQueue();
    } catch {
      setActionQueueError("審批失敗：無法連線至會員管理 BFF。");
    } finally {
      setActionReviewLoading(null);
    }
  }

  return (
    <div className="space-y-6">
      {showMemberWorkspace && (
        <>
          <div className="grid gap-6 xl:grid-cols-[360px_minmax(0,1fr)] xl:items-start">
            <div className="space-y-6">
              <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
                <div className="flex flex-col gap-4">
                  <div>
                    <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                      會員查詢
                    </h2>
                    <p className="mt-2 max-w-2xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                      以會員 ID
                      查詢聯絡資料遮罩、帳戶點數、風險狀態、限制與待處理案件。
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

                <form
                  className="mt-5 flex flex-col gap-3"
                  onSubmit={lookupMember}
                >
                  <label className="relative min-w-0 flex-1">
                    <span className="text-[13px] font-semibold text-proph-ink-muted-80">
                      會員 ID 或姓名
                    </span>
                    <input
                      value={lookupUserId}
                      onChange={(event) => {
                        setLookupUserId(event.target.value);
                        setSuggestionsOpen(true);
                      }}
                      onFocus={() => setSuggestionsOpen(true)}
                      onBlur={() => setSuggestionsOpen(false)}
                      autoComplete="off"
                      role="combobox"
                      aria-autocomplete="list"
                      aria-expanded={
                        suggestionsOpen && memberSuggestions.length > 0
                      }
                      className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus"
                    />
                    {suggestionsOpen && memberSuggestions.length > 0 && (
                      <ul
                        role="listbox"
                        className="absolute inset-x-0 top-full z-20 mt-1 max-h-72 overflow-y-auto rounded-proph-sm border border-proph-hairline bg-proph-canvas py-1 shadow-lg"
                      >
                        {memberSuggestions.map((entry) => (
                          <li
                            key={entry.member.user_id}
                            role="option"
                            aria-selected={false}
                          >
                            <button
                              type="button"
                              onMouseDown={(event) => event.preventDefault()}
                              onClick={() =>
                                selectSuggestion(entry.member.user_id)
                              }
                              className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left hover:bg-proph-surface-pearl"
                            >
                              <span className="truncate text-[14px] text-proph-ink">
                                {entry.member.display_name}
                              </span>
                              <span className="shrink-0 font-mono text-[12px] text-proph-ink-muted-48">
                                {entry.member.user_id}
                              </span>
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
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

              <MemberDirectory
                canDelete={canDelete}
                loading={memberListLoading}
                members={members}
                selectedUserId={summary?.member.user_id}
                viewerRole={viewerRole}
                onCreate={openCreateMemberEditor}
                onEdit={openEditMemberEditor}
                onRefresh={refreshMembers}
                onView={selectMember}
              />
              {memberListError && <Alert tone="error">{memberListError}</Alert>}
            </div>

            <div className="space-y-6 xl:sticky xl:top-6">
              {memberMutationError && (
                <Alert tone="error">{memberMutationError}</Alert>
              )}
              {memberMutationResult && (
                <Alert tone="success">{memberMutationResult}</Alert>
              )}

              {summary ? (
                <>
                  <MemberOverview summary={summary} />
                  <OpenCases cases={summary.open_cases} />
                  {canSubmit && (
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
                  )}
                  <AuditPanel summary={summary} />
                </>
              ) : (
                <section className="rounded-proph-lg border border-dashed border-proph-divider-soft bg-proph-surface-pearl p-6 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  輸入或從左側名單點選會員後，即可檢視聯絡、帳戶、狀態與提交處理動作。
                </section>
              )}
            </div>
          </div>

          {memberEditorOpen && (
            <div
              aria-modal="true"
              className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-proph-ink/40 px-4 py-8"
              role="dialog"
            >
              <div className="w-full max-w-4xl rounded-proph-lg bg-proph-canvas shadow-xl">
                <div className="flex items-center justify-between gap-3 border-b border-proph-hairline px-6 py-4">
                  <h2 className="text-[17px] font-semibold leading-[1.24] tracking-[-0.224px]">
                    {editingUserId ? "編輯會員" : "新增會員"}
                  </h2>
                  <button
                    type="button"
                    onClick={() => setMemberEditorOpen(false)}
                    disabled={memberMutationLoading}
                    className="inline-flex h-9 items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-4 text-[13px] font-semibold text-proph-ink disabled:opacity-50"
                  >
                    關閉
                  </button>
                </div>
                <div className="p-6">
                  <MemberEditor
                    canDelete={canDelete}
                    editingUserId={editingUserId}
                    form={memberForm}
                    loading={memberMutationLoading}
                    mutationResult={null}
                    mutationError={null}
                    selectedSummary={summary}
                    viewerRole={viewerRole}
                    onDelete={deleteSelectedMember}
                    onReset={resetMemberForm}
                    onSubmit={submitMemberMutation}
                    onUpdate={updateMemberForm}
                  />
                </div>
              </div>
            </div>
          )}
        </>
      )}

      {showPendingCases && (
        <PendingCasesPanel
          cases={pendingCases}
          error={pendingCasesError}
          loading={pendingCasesLoading}
          query={pendingQuery}
          restriction={pendingRestriction}
          status={pendingStatus}
          onNextPage={() => setPendingPage((page) => page + 1)}
          onPreviousPage={() => setPendingPage((page) => Math.max(1, page - 1))}
          onQueryChange={setPendingQuery}
          onRefresh={() => {
            setPendingPage(1);
            void refreshPendingCases(1);
          }}
          onRestrictionChange={(value) => {
            setPendingRestriction(value);
            setPendingPage(1);
          }}
          onStatusChange={(value) => {
            setPendingStatus(value);
            setPendingPage(1);
          }}
        />
      )}

      {canSubmit && showActionQueue && (
        <ActionQueuePanel
          actions={actionQueue?.actions ?? []}
          error={actionQueueError}
          loading={actionQueueLoading}
          reviewLoading={actionReviewLoading}
          reviewReasons={actionReviewReasons}
          onRefresh={refreshActionQueue}
          onReview={reviewAction}
          onReviewReasonChange={(actionId, value) =>
            setActionReviewReasons((current) => ({
              ...current,
              [actionId]: value,
            }))
          }
        />
      )}
    </div>
  );
}

function PendingCasesPanel(props: {
  cases: MemberPendingCasesResponse | null;
  error: string | null;
  loading: boolean;
  query: string;
  restriction: string;
  status: MemberStatus | "all";
  onNextPage: () => void;
  onPreviousPage: () => void;
  onQueryChange: (value: string) => void;
  onRefresh: () => void;
  onRestrictionChange: (value: string) => void;
  onStatusChange: (value: MemberStatus | "all") => void;
}) {
  const page = props.cases?.page ?? 1;
  const pageSize = props.cases?.page_size ?? 5;
  const total = props.cases?.total ?? 0;
  const hasNext = page * pageSize < total;
  const sourceLabel =
    props.cases?.source === "api-gateway" ? "api-gateway" : "dev fallback";

  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
            會員待處理 API
          </h2>
          <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            可視化會員 ID、限制原因、聯絡資料與狀態；production 必須讀取 API
            gateway，只有明確開啟 dev fallback 才會使用本機資料。
          </p>
        </div>
        <span className="inline-flex h-8 items-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-3 font-mono text-[12px] text-proph-ink-muted-80">
          source: {sourceLabel}
        </span>
      </div>

      <div className="mt-5 grid gap-3 lg:grid-cols-[1fr_180px_220px_auto]">
        <input
          value={props.query}
          onChange={(event) => props.onQueryChange(event.target.value)}
          className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] outline-none focus:border-proph-primary-focus"
          placeholder="搜尋會員 ID / 聯絡資料"
        />
        <select
          value={props.status}
          onChange={(event) =>
            props.onStatusChange(event.target.value as MemberStatus | "all")
          }
          className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] outline-none focus:border-proph-primary-focus"
        >
          <option value="all">全部狀態</option>
          <option value="inactive">未激活</option>
          <option value="restricted">限制中</option>
          <option value="suspended">停權</option>
          <option value="active">正常</option>
        </select>
        <input
          value={props.restriction}
          onChange={(event) => props.onRestrictionChange(event.target.value)}
          className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] outline-none focus:border-proph-primary-focus"
          placeholder="限制原因"
        />
        <button
          type="button"
          onClick={props.onRefresh}
          disabled={props.loading}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold text-proph-on-primary disabled:opacity-50"
        >
          {props.loading ? "載入中" : "套用"}
        </button>
      </div>

      {props.error && <Alert tone="error">{props.error}</Alert>}

      <div className="mt-5 overflow-hidden rounded-proph-lg border border-proph-hairline">
        <div className="grid grid-cols-[1fr_1.1fr_1.1fr_0.8fr] bg-proph-canvas-parchment px-4 py-3 text-[12px] font-semibold text-proph-ink-muted-80">
          <span>會員 ID</span>
          <span>限制原因</span>
          <span>聯絡資料</span>
          <span>狀態</span>
        </div>
        {props.cases?.items.length ? (
          props.cases.items.map((item) => (
            <div
              key={`${item.member_id}-${item.restriction_reason}`}
              className="grid grid-cols-[1fr_1.1fr_1.1fr_0.8fr] gap-3 border-t border-proph-hairline bg-proph-canvas px-4 py-3 text-[13px]"
            >
              <span className="font-mono text-proph-ink">{item.member_id}</span>
              <span className="font-mono text-proph-ink-muted-80">
                {item.restriction_reason}
              </span>
              <span className="font-mono text-proph-ink-muted-80">
                {item.contact.email_masked} / {item.contact.phone_masked}
              </span>
              <span>
                {statusLabels[item.status]} / {item.kyc_status}
              </span>
            </div>
          ))
        ) : (
          <p className="border-t border-proph-hairline bg-proph-canvas px-4 py-4 text-[14px] text-proph-ink-muted-80">
            沒有符合條件的待處理會員。
          </p>
        )}
      </div>

      <div className="mt-4 flex flex-wrap items-center justify-between gap-3 text-[13px] text-proph-ink-muted-80">
        <span>
          第 {page} 頁，每頁 {pageSize} 筆，共 {total} 筆
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={props.onPreviousPage}
            disabled={page <= 1 || props.loading}
            className="rounded-proph-pill border border-proph-hairline px-4 py-2 disabled:opacity-50"
          >
            上一頁
          </button>
          <button
            type="button"
            onClick={props.onNextPage}
            disabled={!hasNext || props.loading}
            className="rounded-proph-pill border border-proph-hairline px-4 py-2 disabled:opacity-50"
          >
            下一頁
          </button>
        </div>
      </div>
    </section>
  );
}

function MemberDirectory(props: {
  canDelete: boolean;
  loading: boolean;
  members: MemberManagementSummary[];
  selectedUserId?: string;
  viewerRole: Role;
  onCreate: () => void;
  onEdit: (summary: MemberManagementSummary) => void;
  onRefresh: () => void;
  onView: (summary: MemberManagementSummary) => void;
}) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-col gap-4">
        <div>
          <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
            會員名單
          </h2>
          <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {props.viewerRole === "affiliate"
              ? "僅顯示歸屬於你代理鏈的會員，可新增與編輯，不能刪除。"
              : "顯示所有會員，並標識直接註冊或由哪個代理帶入。"}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={props.onCreate}
            className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-4 text-[14px] font-semibold text-proph-on-primary"
          >
            新增會員
          </button>
          <button
            type="button"
            onClick={props.onRefresh}
            disabled={props.loading}
            className="inline-flex h-11 items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-4 text-[14px] font-semibold text-proph-ink disabled:opacity-50"
          >
            {props.loading ? "更新中" : "重新整理"}
          </button>
        </div>
      </div>

      <div className="mt-5 grid gap-3">
        {props.members.length === 0 ? (
          <p className="rounded-proph-sm border border-dashed border-proph-divider-soft bg-proph-surface-pearl p-4 text-[14px] text-proph-ink-muted-80">
            目前沒有可管理的會員。
          </p>
        ) : (
          props.members.map((member) => (
            <article
              key={member.member.user_id}
              role="button"
              tabIndex={0}
              aria-pressed={member.member.user_id === props.selectedUserId}
              onClick={() => props.onView(member)}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  props.onView(member);
                }
              }}
              className={`grid cursor-pointer gap-3 rounded-proph-sm border bg-proph-surface-pearl p-4 lg:grid-cols-[1fr_auto] ${
                member.member.user_id === props.selectedUserId
                  ? "border-proph-primary"
                  : "border-proph-divider-soft hover:border-proph-primary-focus"
              }`}
            >
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusPill
                    tone={
                      member.member.status === "active" ? "success" : "neutral"
                    }
                  >
                    {statusLabels[member.member.status]}
                  </StatusPill>
                  <span className="font-mono text-[12px] text-proph-ink-muted-48">
                    {member.member.user_id}
                  </span>
                </div>
                <h3 className="mt-2 text-[17px] font-semibold leading-[1.24] tracking-[-0.224px]">
                  {member.member.display_name}
                </h3>
                <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  客戶來源：{member.source.affiliate_name}
                  {member.source.affiliate_id ? (
                    <>
                      {" "}
                      (
                      <Link
                        href={`/affiliate/${encodeURIComponent(member.source.affiliate_id)}`}
                        onClick={(event) => event.stopPropagation()}
                        className="font-mono text-proph-primary underline-offset-4 hover:underline"
                      >
                        {member.source.affiliate_id}
                      </Link>
                      )
                    </>
                  ) : null}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2 lg:justify-end">
                <button
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation();
                    props.onEdit(member);
                  }}
                  className="inline-flex h-10 items-center justify-center rounded-proph-pill bg-proph-primary px-4 text-[13px] font-semibold text-proph-on-primary"
                >
                  編輯
                </button>
                {props.canDelete && (
                  <span className="text-[12px] text-proph-ink-muted-48">
                    可刪除
                  </span>
                )}
              </div>
            </article>
          ))
        )}
      </div>
    </section>
  );
}

function ActionQueuePanel(props: {
  actions: MemberActionRecord[];
  error: string | null;
  loading: boolean;
  reviewLoading: string | null;
  reviewReasons: Record<string, string>;
  onRefresh: () => void;
  onReview: (actionId: string, decision: "approve" | "reject") => void;
  onReviewReasonChange: (actionId: string, value: string) => void;
}) {
  const pending = props.actions.filter(
    (action) => action.status === "pending_second_approval",
  );
  const history = props.actions.filter(
    (action) => action.status !== "pending_second_approval",
  );

  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
            工單審批
          </h2>
          <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            待二次審批的會員操作在此核准或駁回；核准後會記錄對應的執行效果。
          </p>
        </div>
        <button
          type="button"
          onClick={props.onRefresh}
          disabled={props.loading}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-4 text-[14px] font-semibold text-proph-ink disabled:opacity-50"
        >
          {props.loading ? "更新中" : "重新整理"}
        </button>
      </div>
      {props.error && <Alert tone="error">{props.error}</Alert>}

      <div className="mt-5 grid gap-4">
        {pending.length === 0 ? (
          <p className="rounded-proph-sm border border-dashed border-proph-divider-soft bg-proph-surface-pearl p-4 text-[14px] text-proph-ink-muted-80">
            目前沒有待二次審批的會員工單。
          </p>
        ) : (
          pending.map((action) => (
            <article
              key={action.action_id}
              className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4"
            >
              <ActionRecordHeader action={action} />
              <textarea
                value={props.reviewReasons[action.action_id] ?? ""}
                onChange={(event) =>
                  props.onReviewReasonChange(
                    action.action_id,
                    event.target.value,
                  )
                }
                className="mt-3 min-h-20 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[14px] outline-none focus:border-proph-primary-focus"
                placeholder="審批原因"
              />
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => props.onReview(action.action_id, "approve")}
                  disabled={props.reviewLoading !== null}
                  className="inline-flex h-10 items-center rounded-proph-pill bg-proph-primary px-4 text-[13px] font-semibold text-proph-on-primary disabled:opacity-50"
                >
                  {props.reviewLoading === `${action.action_id}:approve`
                    ? "核准中"
                    : "核准"}
                </button>
                <button
                  type="button"
                  onClick={() => props.onReview(action.action_id, "reject")}
                  disabled={props.reviewLoading !== null}
                  className="inline-flex h-10 items-center rounded-proph-pill border border-proph-feedback-error px-4 text-[13px] font-semibold text-proph-feedback-error disabled:opacity-50"
                >
                  {props.reviewLoading === `${action.action_id}:reject`
                    ? "駁回中"
                    : "駁回"}
                </button>
              </div>
            </article>
          ))
        )}
      </div>

      {history.length > 0 && (
        <div className="mt-6">
          <h3 className="text-[15px] font-semibold text-proph-ink">
            近期工單歷史
          </h3>
          <div className="mt-3 grid gap-3">
            {history.slice(0, 8).map((action) => (
              <article
                key={action.action_id}
                className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4"
              >
                <ActionRecordHeader action={action} />
                {action.review && (
                  <p className="mt-2 text-[13px] text-proph-ink-muted-80">
                    審批：{action.review.decision} /{" "}
                    {action.review.reviewer_user_id} / {action.review.reason}
                  </p>
                )}
                {action.side_effects.length > 0 && (
                  <p className="mt-2 font-mono text-[12px] text-proph-ink-muted-48">
                    effects: {action.side_effects.join(", ")}
                  </p>
                )}
              </article>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function ActionRecordHeader({ action }: { action: MemberActionRecord }) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div>
        <h3 className="font-mono text-[13px] font-semibold text-proph-ink">
          {action.action_id}
        </h3>
        <p className="mt-1 text-[14px] text-proph-ink-muted-80">
          {actionLabels[action.action_type]} / {action.target_user_id} /{" "}
          {action.ticket_id}
        </p>
        <p className="mt-1 text-[13px] text-proph-ink-muted-48">
          建立者：{action.actor_user_id} / {action.reason}
        </p>
      </div>
      <StatusPill tone={actionTone(action.status)}>{action.status}</StatusPill>
    </div>
  );
}

function actionTone(status: MemberActionStatus) {
  if (status === "approved" || status === "submitted") return "success";
  if (status === "rejected") return "critical";
  return "warning";
}

function MemberEditor(props: {
  canDelete: boolean;
  editingUserId: string | null;
  form: MemberFormState;
  loading: boolean;
  mutationError: string | null;
  mutationResult: string | null;
  selectedSummary: MemberManagementSummary | null;
  viewerRole: Role;
  onDelete: () => void;
  onReset: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onUpdate: <K extends keyof MemberFormState>(
    key: K,
    value: MemberFormState[K],
  ) => void;
}) {
  const affiliateLocked = props.viewerRole === "affiliate";

  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
            {props.editingUserId ? "編輯會員" : "新增會員"}
          </h2>
          <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            新增會員會先建立為未激活；KYC 狀態為 verified
            且風險為低風險時才會自動激活。
          </p>
        </div>
        <button
          type="button"
          onClick={props.onReset}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-surface-pearl px-4 text-[14px] font-semibold text-proph-ink"
        >
          重置
        </button>
      </div>

      <form className="mt-5 grid gap-4" onSubmit={props.onSubmit}>
        <div className="grid gap-4 lg:grid-cols-2">
          <TextField
            disabled={Boolean(props.editingUserId) || props.loading}
            label="會員 ID"
            value={props.form.user_id}
            onChange={(value) => props.onUpdate("user_id", value)}
          />
          <TextField
            disabled={props.loading}
            label="顯示名稱"
            value={props.form.display_name}
            onChange={(value) => props.onUpdate("display_name", value)}
          />
          <TextField
            disabled={props.loading}
            label={props.editingUserId ? "Email（留空不更新）" : "Email"}
            value={props.form.email}
            onChange={(value) => props.onUpdate("email", value)}
          />
          <TextField
            disabled={props.loading}
            label={props.editingUserId ? "手機（留空不更新）" : "手機"}
            value={props.form.phone}
            onChange={(value) => props.onUpdate("phone", value)}
          />
          <TextField
            disabled={props.loading}
            label="LINE ID"
            value={props.form.line_id}
            onChange={(value) => props.onUpdate("line_id", value)}
          />
          <TextField
            disabled={props.loading || affiliateLocked}
            label="客戶來源代理 ID"
            value={props.form.affiliate_id}
            onChange={(value) => props.onUpdate("affiliate_id", value)}
          />
          <TextField
            disabled={props.loading}
            label="地區"
            value={props.form.country}
            onChange={(value) => props.onUpdate("country", value)}
          />
          <TextField
            disabled={props.loading}
            label="時區"
            value={props.form.timezone}
            onChange={(value) => props.onUpdate("timezone", value)}
          />
          <TextField
            disabled={props.loading}
            label="會員等級"
            value={props.form.tier}
            onChange={(value) => props.onUpdate("tier", value)}
          />
          <TextField
            disabled={props.loading}
            label="風險分數"
            value={props.form.risk_score}
            onChange={(value) => props.onUpdate("risk_score", value)}
          />
          <SelectField
            disabled={props.loading}
            label="KYC 狀態"
            value={props.form.kyc_status}
            options={[
              ["pending", "pending"],
              ["review_required", "review_required"],
              ["verified", "verified"],
            ]}
            onChange={(value) => props.onUpdate("kyc_status", value)}
          />
          <SelectField
            disabled={props.loading}
            label="風險等級"
            value={props.form.risk_level}
            options={[
              ["low", "low"],
              ["medium", "medium"],
              ["high", "high"],
            ]}
            onChange={(value) => props.onUpdate("risk_level", value)}
          />
        </div>

        <div className="flex flex-wrap gap-3">
          <button
            type="submit"
            disabled={props.loading}
            className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold text-proph-on-primary disabled:opacity-50"
          >
            {props.loading
              ? "儲存中"
              : props.editingUserId
                ? "儲存變更"
                : "新增會員"}
          </button>
          {props.canDelete && props.selectedSummary && (
            <button
              type="button"
              onClick={props.onDelete}
              disabled={props.loading}
              className="inline-flex h-11 items-center justify-center rounded-proph-pill border border-proph-feedback-error bg-proph-canvas px-5 text-[14px] font-semibold text-proph-feedback-error disabled:opacity-50"
            >
              刪除會員
            </button>
          )}
        </div>
      </form>

      {props.mutationError && <Alert tone="error">{props.mutationError}</Alert>}
      {props.mutationResult && (
        <Alert tone="success">{props.mutationResult}</Alert>
      )}
    </section>
  );
}

function MemberOverview({ summary }: { summary: MemberManagementSummary }) {
  const statusTone = summary.member.status === "active" ? "success" : "warning";
  const riskTone =
    summary.kyc_risk.risk_level === "high" ? "critical" : "neutral";

  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-wrap items-center gap-3">
        <StatusPill tone={statusTone}>
          {statusLabels[summary.member.status]}
        </StatusPill>
        <StatusPill tone={riskTone}>
          {riskLabels[summary.kyc_risk.risk_level]}
        </StatusPill>
        <span className="font-mono text-[12px] text-proph-ink-muted-48">
          {summary.member.user_id}
        </span>
      </div>
      <h2 className="mt-4 text-[24px] font-semibold leading-[1.15] tracking-[-0.224px]">
        {summary.member.display_name}
      </h2>
      <p className="mt-1 text-[14px] text-proph-ink-muted-80">
        {summary.member.tier} / 註冊於{" "}
        {formatDateTime(summary.member.registered_at)}
      </p>

      <div className="mt-6 grid gap-4 lg:grid-cols-2">
        <InfoCard title="聯絡資料">
          <Metric label="Email" value={summary.member.contact.email_masked} />
          <Metric label="手機" value={summary.member.contact.phone_masked} />
          <Metric
            label="LINE"
            value={summary.member.contact.line_id_masked ?? "未綁定"}
          />
          <Metric
            label="地區"
            value={`${summary.member.contact.country} / ${summary.member.contact.timezone}`}
          />
        </InfoCard>
        <InfoCard title="帳戶摘要">
          <Metric
            label="可用點數"
            value={formatMinorUnitsAsMemberPoints(
              summary.account.available_minor_units,
            )}
          />
          <Metric
            label="鎖定點數"
            value={formatMinorUnitsAsMemberPoints(
              summary.account.locked_minor_units,
            )}
          />
          <Metric
            label="凍結點數"
            value={formatMinorUnitsAsMemberPoints(
              summary.account.frozen_minor_units,
            )}
          />
          <Metric
            label="2FA"
            value={summary.account.two_factor_enabled ? "已啟用" : "未啟用"}
          />
        </InfoCard>
        <InfoCard title="狀態與風控">
          <Metric label="KYC" value={summary.kyc_risk.kyc_status} />
          <Metric
            label="風險分數"
            value={String(summary.kyc_risk.risk_score)}
          />
          <Metric
            label="限制"
            value={summary.kyc_risk.restrictions.join(", ") || "無"}
          />
          <Metric
            label="最近覆核"
            value={formatOptionalDateTime(summary.kyc_risk.last_reviewed_at)}
          />
        </InfoCard>
        <InfoCard title="KYC 證件材料">
          <KycDocumentGallery userId={summary.member.user_id} />
        </InfoCard>
        <InfoCard title="登入與裝置">
          <Metric
            label="最近登入"
            value={formatOptionalDateTime(summary.account.last_login_at)}
          />
          <Metric label="裝置數" value={`${summary.account.device_count} 台`} />
          <Metric label="幣別" value={summary.account.currency} />
          <Metric
            label="會員狀態"
            value={statusLabels[summary.member.status]}
          />
          <Metric label="客戶來源" value={summary.source.affiliate_name} />
        </InfoCard>
      </div>
    </section>
  );
}

function OpenCases({
  cases,
}: {
  cases: MemberManagementSummary["open_cases"];
}) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        待處理案件
      </h2>
      {cases.length === 0 ? (
        <p className="mt-4 text-[14px] text-proph-ink-muted-80">
          目前沒有未結案件。
        </p>
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
                <StatusPill
                  tone={item.status === "blocked" ? "warning" : "neutral"}
                >
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
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">
            動作
          </span>
          <select
            value={props.actionType}
            onChange={(event) =>
              props.onActionTypeChange(event.target.value as MemberActionType)
            }
            disabled={!props.canSubmit || props.actionLoading}
            className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
          >
            {Object.entries(actionLabels).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">
            工單 ID
          </span>
          <input
            value={props.ticketId}
            onChange={(event) => props.onTicketIdChange(event.target.value)}
            disabled={!props.canSubmit || props.actionLoading}
            className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
            placeholder="OPS-1042"
          />
        </label>
        <label className="block">
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">
            處理原因
          </span>
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
          已提交 {props.actionResult.action_id}，狀態：
          {props.actionResult.status}，correlation id：
          {props.actionResult.audit.correlation_id}
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
        <Metric
          label="需稽核"
          value={summary.audit.audit_required ? "是" : "否"}
        />
        <Metric
          label="最近管理動作"
          value={formatOptionalDateTime(summary.audit.latest_admin_action_at)}
        />
      </div>
    </section>
  );
}

function InfoCard({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4">
      <h3 className="text-[15px] font-semibold leading-[1.29] text-proph-ink">
        {title}
      </h3>
      <dl className="mt-3 grid gap-2">{children}</dl>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-4 rounded-proph-sm border border-proph-divider-soft bg-proph-canvas px-3 py-2">
      <dt className="text-[12px] font-semibold text-proph-ink-muted-80">
        {label}
      </dt>
      <dd className="text-right font-mono text-[13px] text-proph-ink">
        {value}
      </dd>
    </div>
  );
}

function TextField(props: {
  disabled?: boolean;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="text-[13px] font-semibold text-proph-ink-muted-80">
        {props.label}
      </span>
      <input
        value={props.value}
        onChange={(event) => props.onChange(event.target.value)}
        disabled={props.disabled}
        className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
      />
    </label>
  );
}

function SelectField<T extends string>(props: {
  disabled?: boolean;
  label: string;
  value: T;
  options: Array<[T, string]>;
  onChange: (value: T) => void;
}) {
  return (
    <label className="block">
      <span className="text-[13px] font-semibold text-proph-ink-muted-80">
        {props.label}
      </span>
      <select
        value={props.value}
        onChange={(event) => props.onChange(event.target.value as T)}
        disabled={props.disabled}
        className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
      >
        {props.options.map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
    </label>
  );
}

function Alert({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "error" | "success";
}) {
  const border =
    tone === "error"
      ? "border-proph-feedback-error"
      : "border-proph-feedback-success";
  return (
    <p
      className={`mt-4 rounded-proph-sm border ${border} bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink`}
    >
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
