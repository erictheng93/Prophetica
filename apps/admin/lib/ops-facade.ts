import "server-only";

import type { SessionPayload } from "./session-crypto.js";
import {
  fetchAffiliatePayoutReviews,
  fetchCommissionAdjustmentReviews,
  type AffiliateCommissionAdjustmentReview,
  type AffiliatePayoutReview,
} from "./affiliate-management-bff.js";
import {
  proxyMemberActionListRequest,
  proxyMemberPendingCasesRequest,
} from "./member-management-bff.js";
import type {
  MemberActionRecord,
  MemberActionStatus,
  MemberPendingCase,
} from "./member-management-shared.js";

export type OpsDomainId =
  | "member-management"
  | "points-exceptions"
  | "affiliate-review"
  | "kyc-risk-events"
  | "compensation-reviews"
  | "manual-approvals";

export type OpsTone = "neutral" | "warning" | "success" | "critical";

export interface OpsRoute {
  href: string;
  label: string;
  domain: OpsDomainId;
  summary: string;
  scope: string;
  owner: string;
  openItems: number;
  tone: OpsTone;
  children?: OpsRouteChild[];
  navHidden?: boolean;
  /**
   * Feature is wired in the UI but its api-gateway backend is not yet
   * connected. Surfaced as a「開發中」nav badge + page banner; flip to
   * false (or remove) once the gateway integration ships.
   */
  inDevelopment?: boolean;
}

export interface OpsRouteChild {
  href: string;
  label: string;
  key: string;
}

export interface OpsQueueItem {
  id: string;
  domain: OpsDomainId;
  title: string;
  detail: string;
  status: "new" | "reviewing" | "blocked" | "ready";
  assignee: string;
  age: string;
}

export interface OpsQueueSummary {
  label: string;
  value: string;
  detail: string;
  tone: OpsTone;
}

export interface CrossDomainStreamAcknowledgement {
  status: "confirmed";
  returnedFrom: OpsDomainId;
  confirmedAt: string;
  receiptId: string;
  returnPath: string;
}

export interface CrossDomainStreamEvent {
  id: string;
  correlationId: string;
  title: string;
  sourceDomain: OpsDomainId;
  targetDomains: OpsDomainId[];
  linkedQueueIds: string[];
  emittedAt: string;
  acknowledgement: CrossDomainStreamAcknowledgement;
}

export interface AdminOperationsSnapshot {
  generatedAt: string;
  routes: OpsRoute[];
  summaries: OpsQueueSummary[];
  queue: OpsQueueItem[];
  crossDomainStream: CrossDomainStreamEvent[];
}

export interface DecisionSupportMetric {
  id: "kpi" | "alerts" | "versioned-reports" | "audit-trail";
  label: string;
  value: string;
  detail: string;
  tone: OpsTone;
}

export interface DecisionSupportReport {
  reportId: string;
  title: string;
  version: string;
  generatedAt: string;
  hash: string;
  owner: string;
}

export interface DecisionSupportSnapshot {
  generatedAt: string;
  metrics: DecisionSupportMetric[];
  reports: DecisionSupportReport[];
  auditQueries: string[];
}

const routes: OpsRoute[] = [
  {
    href: "/members",
    label: "會員管理",
    domain: "member-management",
    summary:
      "管理會員資料、帳戶風險與來源歸屬，並支援 KYC 審核流程。會員管理與 KYC 審核已拆分為子頁面。",
    scope:
      "客服、營運與授權代理可在此做會員查詢、資料更新、待處理案件回顧；新會員需先完成 KYC 綠燈才會激活。",
    owner: "Member Ops",
    openItems: 0,
    tone: "neutral",
    children: [
      { key: "member-list", href: "/members", label: "會員清單" },
      {
        key: "member-kyc-review",
        href: "/members/kyc-review",
        label: "KYC 審核",
      },
      {
        key: "member-action-approvals",
        href: "/approvals",
        label: "工單審批",
      },
    ],
  },
  {
    href: "/points",
    label: "帳戶點數例外",
    domain: "points-exceptions",
    summary: "追蹤點數入帳延遲、人工調整、凍結與對帳例外。",
    scope:
      "所有點數補償與修正需經雙人審批；此入口只建立審查脈絡，不直接寫入帳本。",
    owner: "Wallet Ops",
    openItems: 0,
    tone: "warning",
  },
  {
    href: "/affiliate",
    label: "代理管理區",
    domain: "affiliate-review",
    summary: "管理代理註冊、管控、佣金與撥款審查。",
    scope:
      "Admin 與 Manager 可管理代理資料、佣金增減與撥款審查；佣金率與撥款處分需保留 audit trail。",
    owner: "Commission Ops",
    openItems: 0,
    tone: "neutral",
    children: [
      { key: "affiliate-list", href: "/affiliate", label: "代理清單" },
      {
        key: "affiliate-commissions",
        href: "/affiliate/commissions",
        label: "佣金管理",
      },
      {
        key: "affiliate-payouts",
        href: "/affiliate/payouts",
        label: "撥款管理",
      },
    ],
  },
  {
    href: "/kyc-risk",
    label: "KYC / 風險事件",
    domain: "kyc-risk-events",
    summary: "覆核 KYC 狀態、制裁命中、PEP、裝置與交易風險事件。",
    scope:
      "高風險案件需由風控與合規共同處理，必要時凍結帳戶功能並保留既有持倉。",
    owner: "Risk Ops",
    openItems: 0,
    tone: "critical",
  },
  {
    href: "/compensation",
    label: "補償審批",
    domain: "compensation-reviews",
    summary: "處理客服補償、服務事件 credit 與人工點數提案。",
    scope:
      "補償提案必須具備 ticket、原因、金額與兩名不同核准人，入帳前不可視為完成。",
    owner: "Support Finance",
    openItems: 0,
    tone: "success",
  },
  {
    href: "/approvals",
    label: "工單審批",
    domain: "manual-approvals",
    summary: "審批待二次核准的會員操作工單。",
    scope:
      "Admin 與 Manager 可在此核准或駁回會員工單；實際會員處分仍由會員管理 BFF 與下游系統端點執行。",
    owner: "Member Ops",
    openItems: 0,
    tone: "warning",
    navHidden: true,
    inDevelopment: true,
  },
];

const mockQueue: OpsQueueItem[] = [
  {
    id: "OPS-1042",
    domain: "kyc-risk-events",
    title: "Tier 2 文件覆核需合規確認",
    detail: "Provider 回傳 YELLOW，需比對會員聲明資料與近期點數活動。",
    status: "new",
    assignee: "Risk Ops",
    age: "24m",
  },
  {
    id: "OPS-1037",
    domain: "points-exceptions",
    title: "活動獎勵延遲入帳補償",
    detail: "客服已附工單與活動紀錄，可建立補償提案但不得直接入帳。",
    status: "ready",
    assignee: "Wallet Ops",
    age: "1h",
  },
  {
    id: "OPS-1029",
    domain: "affiliate-review",
    title: "推薦鏈佣金異常覆核",
    detail: "同一推薦鏈出現密集轉換，需確認是否符合夥伴層級規則。",
    status: "reviewing",
    assignee: "Commission Ops",
    age: "3h",
  },
  {
    id: "OPS-1018",
    domain: "manual-approvals",
    title: "會員升級工單等待第二核准",
    detail: "涉及會員帳戶限制或升級處理，需二次核准後才可進入處理。",
    status: "blocked",
    assignee: "Member Ops",
    age: "5h",
  },
];

const crossDomainStreamTemplates: Omit<
  CrossDomainStreamEvent,
  "linkedQueueIds"
>[] = [
  {
    id: "stream-kyc-risk-member-review",
    correlationId: "corr-ops-1042",
    title: "KYC 風險事件同步會員覆核與工單審批",
    sourceDomain: "kyc-risk-events",
    targetDomains: ["member-management", "manual-approvals"],
    emittedAt: "2026-05-16T09:12:00+08:00",
    acknowledgement: {
      status: "confirmed",
      returnedFrom: "manual-approvals",
      confirmedAt: "2026-05-16T09:12:04+08:00",
      receiptId: "ack-ops-1042-manual-approvals",
      returnPath: "manual-approvals -> kyc-risk-events",
    },
  },
  {
    id: "stream-manual-approval-points-compensation",
    correlationId: "corr-ops-1018",
    title: "跨領域處分同步點數例外與補償審批",
    sourceDomain: "manual-approvals",
    targetDomains: ["points-exceptions", "compensation-reviews"],
    emittedAt: "2026-05-16T09:14:00+08:00",
    acknowledgement: {
      status: "confirmed",
      returnedFrom: "points-exceptions",
      confirmedAt: "2026-05-16T09:14:03+08:00",
      receiptId: "ack-ops-1018-points-exceptions",
      returnPath: "points-exceptions -> manual-approvals",
    },
  },
  {
    id: "stream-affiliate-review-payout-control",
    correlationId: "corr-ops-1029",
    title: "代理審查同步工單審批與補償控制",
    sourceDomain: "affiliate-review",
    targetDomains: ["manual-approvals", "compensation-reviews"],
    emittedAt: "2026-05-16T09:16:00+08:00",
    acknowledgement: {
      status: "confirmed",
      returnedFrom: "compensation-reviews",
      confirmedAt: "2026-05-16T09:16:05+08:00",
      receiptId: "ack-ops-1029-compensation-reviews",
      returnPath: "compensation-reviews -> affiliate-review",
    },
  },
];

const decisionSupportMetrics: DecisionSupportMetric[] = [
  {
    id: "kpi",
    label: "KPI",
    value: "99.92%",
    detail: "審批服務可用性與 SLA 均在門檻內。",
    tone: "success",
  },
  {
    id: "alerts",
    label: "警示",
    value: "9",
    detail: "高風險 KYC / 點數事件需要升級。",
    tone: "critical",
  },
  {
    id: "versioned-reports",
    label: "版本化報表",
    value: "12",
    detail: "本日已產生並鎖定雜湊的管理報表。",
    tone: "neutral",
  },
  {
    id: "audit-trail",
    label: "稽核覆蓋",
    value: "100%",
    detail: "審批、補償與風控事件均有 correlation id。",
    tone: "warning",
  },
];

const decisionSupportReports: DecisionSupportReport[] = [
  {
    reportId: "RPT-OPS-2026-05-10",
    title: "營運審批 KPI",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:00:00+08:00",
    hash: "sha256:ops-kpi-preview",
    owner: "Ops Control",
  },
  {
    reportId: "RPT-RISK-2026-05-10",
    title: "風控警示摘要",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:15:00+08:00",
    hash: "sha256:risk-alert-preview",
    owner: "Risk Ops",
  },
  {
    reportId: "RPT-AUDIT-2026-05-10",
    title: "稽核查詢清單",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:30:00+08:00",
    hash: "sha256:audit-query-preview",
    owner: "Compliance",
  },
];

export function getAdminOperationsSnapshot(): AdminOperationsSnapshot {
  const queue = [...mockQueue];

  return {
    generatedAt: "2026-05-10T00:00:00+08:00",
    routes: buildRoutesWithQueueMetrics(queue),
    summaries: buildQueueSummaries(queue),
    queue,
    crossDomainStream: buildCrossDomainStream(queue),
  };
}

export async function getAdminOperationsLiveSnapshot(
  viewer: SessionPayload,
): Promise<AdminOperationsSnapshot> {
  const [
    rawPendingCases,
    rawMemberActions,
    rawCommissionAdjustments,
    rawAffiliatePayouts,
  ] = await Promise.all([
    safeCall(() => fetchMemberPendingCases(viewer)),
    safeCall(() => fetchMemberActions(viewer)),
    safeCall(() => fetchCommissionAdjustmentReviews(viewer)),
    safeCall(() => fetchAffiliatePayoutReviews(viewer)),
  ]);

  const pendingCases = rawPendingCases ?? [];
  const memberActions = rawMemberActions ?? [];
  const commissionAdjustments = rawCommissionAdjustments ?? [];
  const affiliatePayouts = rawAffiliatePayouts ?? [];

  const candidates: Array<OpsQueueItem & { sortEpoch: number }> = [
    ...pendingCases.map((item) => mapPendingCaseToQueueItem(item)),
    ...memberActions.map((item) => mapMemberActionToQueueItem(item)),
    ...commissionAdjustments.map((item) =>
      mapCommissionAdjustmentToQueueItem(item),
    ),
    ...affiliatePayouts.map((item) => mapAffiliatePayoutToQueueItem(item)),
  ].sort((a, b) => b.sortEpoch - a.sortEpoch);

  const queue = candidates.map(({ sortEpoch: _sortEpoch, ...item }) => item);

  return {
    generatedAt: new Date().toISOString(),
    routes: buildRoutesWithQueueMetrics(queue),
    summaries: buildQueueSummaries(queue),
    queue: queue.slice(0, 30),
    crossDomainStream: buildCrossDomainStream(queue),
  };
}

export function getAdminOperationsRoutes(): OpsRoute[] {
  return routes;
}

export function getOpsRoute(domain: OpsDomainId): OpsRoute {
  const route = routes.find((item) => item.domain === domain);

  if (!route) {
    throw new Error(`Unknown admin operations domain: ${domain}`);
  }

  return route;
}

export function getDecisionSupportSnapshot(): DecisionSupportSnapshot {
  return {
    generatedAt: "2026-05-10T08:30:00+08:00",
    metrics: decisionSupportMetrics,
    reports: decisionSupportReports,
    auditQueries: [
      "admin.actions.by_actor",
      "compensation.proposals.by_ticket",
      "affiliate.rate_changes.by_scope",
      "kyc_risk.events.by_member",
    ],
  };
}

interface QueueItemWithSort extends OpsQueueItem {
  sortEpoch: number;
}

function buildRoutesWithQueueMetrics(queue: OpsQueueItem[]): OpsRoute[] {
  const counts = countQueueItemsByDomain(queue);

  return routes.map((route) => {
    const domainItems = queue.filter((item) => item.domain === route.domain);

    return {
      ...route,
      openItems: counts[route.domain],
      tone: toneForDomainQueue(route.domain, domainItems),
    };
  });
}

function countQueueItemsByDomain(
  queue: OpsQueueItem[],
): Record<OpsDomainId, number> {
  const counts = Object.fromEntries(
    routes.map((route) => [route.domain, 0]),
  ) as Record<OpsDomainId, number>;

  for (const item of queue) {
    counts[item.domain] += 1;
  }

  return counts;
}

function toneForDomainQueue(
  domain: OpsDomainId,
  items: OpsQueueItem[],
): OpsTone {
  if (items.length === 0) {
    return "success";
  }
  if (domain === "kyc-risk-events") {
    return "critical";
  }
  if (items.some((item) => item.status === "blocked")) {
    return "warning";
  }
  if (items.some((item) => item.status === "reviewing")) {
    return "warning";
  }
  if (items.every((item) => item.status === "ready")) {
    return "success";
  }
  return "neutral";
}

async function fetchMemberPendingCases(
  viewer: SessionPayload,
): Promise<MemberPendingCase[]> {
  const params = new URLSearchParams({
    page_size: "50",
    page: "1",
  });
  const response = await proxyMemberPendingCasesRequest({
    viewer,
    searchParams: params,
  });

  if (!response?.ok) {
    return [];
  }

  try {
    const body = (await response.json()) as {
      items: MemberPendingCase[];
    };
    return body.items ?? [];
  } catch {
    return [];
  }
}

async function fetchMemberActions(
  viewer: SessionPayload,
): Promise<MemberActionRecord[]> {
  const params = new URLSearchParams({ status: "pending_second_approval" });
  const response = await proxyMemberActionListRequest({
    viewer,
    searchParams: params,
  });

  if (!response?.ok) {
    return [];
  }

  try {
    const body = (await response.json()) as { actions: MemberActionRecord[] };
    return body.actions ?? [];
  } catch {
    return [];
  }
}

function mapPendingCaseToQueueItem(
  caseItem: MemberPendingCase,
): QueueItemWithSort {
  const domain: OpsDomainId =
    caseItem.kyc_status === "review_required" ||
    caseItem.kyc_status === "pending"
      ? "kyc-risk-events"
      : caseItem.status === "restricted" || caseItem.status === "suspended"
        ? "points-exceptions"
        : "member-management";

  return {
    id: `case-${caseItem.member_id}`,
    domain,
    title: `會員待處理：${caseItem.display_name} (${caseItem.member_id})`,
    detail: `${caseItem.restriction_reason}（${caseItem.contact.email_masked} / ${caseItem.contact.phone_masked}）`,
    status: mapPendingCaseStatus(caseItem.status),
    assignee: "KYC / 風控小組",
    age: "待分配",
    sortEpoch: Number.POSITIVE_INFINITY,
  };
}

function mapMemberActionToQueueItem(
  action: MemberActionRecord,
): QueueItemWithSort {
  const domain = mapActionDomain(action.action_type);

  return {
    id: action.action_id,
    domain,
    title: `${action.ticket_id}：${actionTitleByType(action.action_type)}`,
    detail: `對象 ${action.target_user_id}，${action.reason}`,
    status: mapActionStatusToQueueStatus(action.status),
    assignee: `${action.actor_role}:${action.actor_user_id}`,
    age: formatRelativeAge(action.created_at),
    sortEpoch: parseSortEpoch(action.created_at),
  };
}

function mapCommissionAdjustmentToQueueItem(
  adjustment: AffiliateCommissionAdjustmentReview,
): QueueItemWithSort {
  return {
    id: adjustment.id,
    domain: "affiliate-review",
    title: `佣金調整：${adjustment.adjustmentType}`,
    detail: `${adjustment.reason}，金額 ${formatMinorUnits(adjustment.amountMinor)} ${adjustment.currency}`,
    status: mapStringStatusToQueueStatus(adjustment.status),
    assignee: adjustment.requestedBy,
    age: formatRelativeAge(adjustment.requestedAt),
    sortEpoch: parseSortEpoch(adjustment.requestedAt),
  };
}

function mapAffiliatePayoutToQueueItem(
  payout: AffiliatePayoutReview,
): QueueItemWithSort {
  return {
    id: payout.id,
    domain: "affiliate-review",
    title: `撥款審批：${payout.id}`,
    detail: `代理提領 ${formatMinorUnits(payout.amountMinor)} ${payout.currency}`,
    status: mapStringStatusToQueueStatus(payout.status),
    assignee: "代理審批組",
    age: formatRelativeAge(payout.requestedAt),
    sortEpoch: parseSortEpoch(payout.requestedAt),
  };
}

function mapPendingCaseStatus(
  status: MemberPendingCase["status"],
): OpsQueueItem["status"] {
  if (status === "suspended") {
    return "blocked";
  }
  if (status === "restricted") {
    return "reviewing";
  }
  return "new";
}

function mapActionDomain(
  actionType: MemberActionRecord["action_type"],
): OpsDomainId {
  if (actionType === "restrict_withdrawals") {
    return "points-exceptions";
  }
  if (actionType === "request_kyc_review") {
    return "kyc-risk-events";
  }
  if (actionType === "escalate_case") {
    return "manual-approvals";
  }
  return "member-management";
}

function mapActionStatusToQueueStatus(
  status: MemberActionStatus,
): OpsQueueItem["status"] {
  if (status === "pending_second_approval") {
    return "reviewing";
  }
  if (status === "approved") {
    return "ready";
  }
  if (status === "rejected") {
    return "blocked";
  }
  return "new";
}

function mapStringStatusToQueueStatus(status: string): OpsQueueItem["status"] {
  if (
    status === "pending_second_approval" ||
    status === "submitted" ||
    status === "pending"
  ) {
    return "reviewing";
  }
  if (status === "approved" || status === "done") {
    return "ready";
  }
  if (status === "rejected" || status === "failed") {
    return "blocked";
  }
  return "new";
}

function actionTitleByType(
  actionType: MemberActionRecord["action_type"],
): string {
  if (actionType === "add_note") {
    return "補充備註";
  }
  if (actionType === "request_kyc_review") {
    return "申請 KYC 覆核";
  }
  if (actionType === "restrict_withdrawals") {
    return "限制提領";
  }
  return "升級案件";
}

function buildQueueSummaries(queue: OpsQueueItem[]): OpsQueueSummary[] {
  const total = queue.length;
  const highRisk = queue.filter(
    (item) => item.domain === "kyc-risk-events" || item.status === "blocked",
  ).length;
  const waiting = queue.filter((item) => item.status === "reviewing").length;
  const ready = queue.filter((item) => item.status === "ready").length;
  const blocked = queue.filter((item) => item.status === "blocked").length;
  const followUp = Math.max(0, queue.length - blocked - ready);

  return [
    {
      label: "未結項目",
      value: String(total),
      detail: total === 0 ? "目前無待處理項目" : "全部操作領域",
      tone: total === 0 ? "success" : "neutral",
    },
    {
      label: "高風險",
      value: String(highRisk),
      detail: "高風險或封鎖事件",
      tone: highRisk === 0 ? "success" : "critical",
    },
    {
      label: "待審批",
      value: String(waiting),
      detail: "等待核可或補件",
      tone: waiting === 0 ? "success" : "warning",
    },
    {
      label: "可處理",
      value: String(ready),
      detail: `${blocked} 件受阻，${followUp} 件需覆核`,
      tone: ready === 0 ? "neutral" : "success",
    },
  ];
}

function buildCrossDomainStream(
  queue: OpsQueueItem[],
): CrossDomainStreamEvent[] {
  return crossDomainStreamTemplates.map((template) => ({
    ...template,
    linkedQueueIds: linkedQueueIdsForDomains(queue, [
      template.sourceDomain,
      ...template.targetDomains,
    ]),
  }));
}

function linkedQueueIdsForDomains(
  queue: OpsQueueItem[],
  domains: OpsDomainId[],
): string[] {
  const domainSet = new Set(domains);
  const linked = queue
    .filter((item) => domainSet.has(item.domain))
    .map((item) => item.id);

  return linked.length > 0
    ? [...new Set(linked)].slice(0, 4)
    : ["stream-control"];
}

function formatMinorUnits(minorUnits: number): string {
  const amount = Math.abs(minorUnits);
  const major = Math.floor(amount / 100);
  const minor = String(amount % 100).padStart(2, "0");
  return `${minorUnits < 0 ? "-" : ""}${major.toLocaleString("zh-Hant")}.${minor}`;
}

function parseSortEpoch(value: string): number {
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? Number.POSITIVE_INFINITY : parsed;
}

function formatRelativeAge(timestamp: string): string {
  const epoch = parseSortEpoch(timestamp);
  if (!Number.isFinite(epoch) || epoch === Number.POSITIVE_INFINITY) {
    return "待處理";
  }

  const deltaSeconds = Math.floor((Date.now() - epoch) / 1000);
  if (deltaSeconds < 60) {
    return "剛剛";
  }
  if (deltaSeconds < 3600) {
    return `${Math.floor(deltaSeconds / 60)}m`;
  }
  if (deltaSeconds < 86400) {
    return `${Math.floor(deltaSeconds / 3600)}h`;
  }
  return `${Math.floor(deltaSeconds / 86400)}d`;
}

async function safeCall<T>(operation: () => Promise<T>): Promise<T | null> {
  try {
    return await operation();
  } catch {
    return null;
  }
}
