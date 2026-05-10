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

export interface AdminOperationsSnapshot {
  generatedAt: string;
  routes: OpsRoute[];
  summaries: OpsQueueSummary[];
  queue: OpsQueueItem[];
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
    summary: "查詢會員、客服脈絡與帳戶狀態。",
    scope: "檢視帳戶資料、風險標記、關聯案件與升級紀錄。",
    owner: "會員營運",
    openItems: 18,
    tone: "neutral",
  },
  {
    href: "/points",
    label: "帳戶點數例外",
    domain: "points-exceptions",
    summary: "處理餘額爭議，但不直接寫入帳本。",
    scope: "建立例外案件、附上證據，並送出帳本覆核請求。",
    owner: "錢包營運",
    openItems: 7,
    tone: "warning",
  },
  {
    href: "/affiliate",
    label: "代理權限與抽佣審查",
    domain: "affiliate-review",
    summary: "審查代理權限、發放暫停與抽佣異常。",
    scope: "檢視權限狀態、抽佣佇列與必要審批。",
    owner: "成長營運",
    openItems: 11,
    tone: "neutral",
  },
  {
    href: "/kyc-risk",
    label: "KYC / 風控事件",
    domain: "kyc-risk-events",
    summary: "監控身分審查、風險訊號與供應商回調。",
    scope: "協調風險事件審查；決策仍須留痕並與執行流程分離。",
    owner: "風控營運",
    openItems: 23,
    tone: "critical",
  },
  {
    href: "/compensation",
    label: "補償審查",
    domain: "compensation-reviews",
    summary: "評估受影響會員的補償請求。",
    scope: "檢視事故關聯、既有補償紀錄與審批要求。",
    owner: "客服主管",
    openItems: 5,
    tone: "success",
  },
  {
    href: "/approvals",
    label: "人工審批",
    domain: "manual-approvals",
    summary: "敏感操作的集中審批佇列。",
    scope: "將審批送往具權限的覆核者；此 stub 不執行即時變更。",
    owner: "營運控制",
    openItems: 14,
    tone: "warning",
  },
];

const summaries: OpsQueueSummary[] = [
  { label: "開放佇列", value: "78", detail: "全部操作域", tone: "neutral" },
  { label: "風險升級", value: "9", detail: "需當日審查", tone: "critical" },
  { label: "可進入審批", value: "16", detail: "等待授權操作者", tone: "warning" },
  { label: "今日已結案", value: "31", detail: "營運快照預覽", tone: "success" },
];

const queue: OpsQueueItem[] = [
  {
    id: "OPS-1042",
    domain: "kyc-risk-events",
    title: "Tier 2 供應商資料不一致",
    detail: "身分回調與會員國別衝突，已關聯風險事件。",
    status: "new",
    assignee: "風控分流",
    age: "24m",
  },
  {
    id: "OPS-1037",
    domain: "points-exceptions",
    title: "點數例外證據已齊備",
    detail: "客服已補交易參照，可送出帳本覆核請求。",
    status: "ready",
    assignee: "錢包覆核",
    age: "1h",
  },
  {
    id: "OPS-1029",
    domain: "affiliate-review",
    title: "抽佣暫停審查",
    detail: "代理權限變更與發放暫停需雙人覆核。",
    status: "reviewing",
    assignee: "成長營運",
    age: "3h",
  },
  {
    id: "OPS-1018",
    domain: "manual-approvals",
    title: "人工審批政策檢查",
    detail: "敏感操作需兩位授權覆核者簽核後才能繼續。",
    status: "blocked",
    assignee: "營運控制",
    age: "5h",
  },
];

const decisionSupportMetrics: DecisionSupportMetric[] = [
  {
    id: "kpi",
    label: "KPI",
    value: "99.92%",
    detail: "審查佇列的營運 SLA 覆蓋率。",
    tone: "success",
  },
  {
    id: "alerts",
    label: "告警",
    value: "9",
    detail: "需當日處理的風險與審批告警。",
    tone: "critical",
  },
  {
    id: "versioned-reports",
    label: "版本化報表",
    value: "12",
    detail: "已發布並保留版本與雜湊的報表快照。",
    tone: "neutral",
  },
  {
    id: "audit-trail",
    label: "留痕查詢",
    value: "100%",
    detail: "儀表板讀取具備稽核基礎且不提供寫入。",
    tone: "warning",
  },
];

const decisionSupportReports: DecisionSupportReport[] = [
  {
    reportId: "RPT-OPS-2026-05-10",
    title: "每日營運 KPI",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:00:00+08:00",
    hash: "sha256:ops-kpi-preview",
    owner: "營運控制",
  },
  {
    reportId: "RPT-RISK-2026-05-10",
    title: "風險告警摘要",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:15:00+08:00",
    hash: "sha256:risk-alert-preview",
    owner: "風控營運",
  },
  {
    reportId: "RPT-AUDIT-2026-05-10",
    title: "留痕查詢證據",
    version: "v2026.05.10.1",
    generatedAt: "2026-05-10T08:30:00+08:00",
    hash: "sha256:audit-query-preview",
    owner: "合規",
  },
];

export function getAdminOperationsSnapshot(): AdminOperationsSnapshot {
  return {
    generatedAt: "2026-05-10T00:00:00+08:00",
    routes,
    summaries,
    queue,
  };
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
