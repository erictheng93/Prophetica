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
    summary: "查詢與管理會員狀態、KYC、來源代理與帳戶摘要。",
    scope:
      "用於客服、營運與代理管理會員基本資料、帳戶狀態、KYC 狀態與來源歸屬；新增會員需先完成 KYC 綠燈才會激活。",
    owner: "Member Ops",
    openItems: 18,
    tone: "neutral",
    children: [
      { key: "member-list", href: "/members", label: "會員清單" },
      {
        key: "member-kyc-review",
        href: "/members/kyc-review",
        label: "KYC 審核",
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
    openItems: 7,
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
    openItems: 11,
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
    label: "KYC / 風控事件",
    domain: "kyc-risk-events",
    summary: "覆核 KYC 狀態、制裁命中、PEP、裝置與交易風險事件。",
    scope:
      "高風險案件需由風控與合規共同處理，必要時凍結帳戶功能並保留既有持倉。",
    owner: "Risk Ops",
    openItems: 23,
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
    openItems: 5,
    tone: "success",
  },
  {
    href: "/approvals",
    label: "人工審批",
    domain: "manual-approvals",
    summary: "集中檢視需要人工核准的跨領域工作。",
    scope: "管理例外流程、升級案件與待核准工作；實際處分由各系統端點執行。",
    owner: "Ops Control",
    openItems: 14,
    tone: "warning",
  },
];

const summaries: OpsQueueSummary[] = [
  { label: "未結項目", value: "78", detail: "全部操作領域", tone: "neutral" },
  { label: "高風險", value: "9", detail: "需合規或風控升級", tone: "critical" },
  { label: "待審批", value: "16", detail: "等待第二核准人", tone: "warning" },
  {
    label: "可處理",
    value: "31",
    detail: "資料齊備，可進入下一步",
    tone: "success",
  },
];

const queue: OpsQueueItem[] = [
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
    title: "跨領域處分等待第二核准",
    detail: "涉及帳戶限制與補償提案，需合規核准後才可進入處理。",
    status: "blocked",
    assignee: "Ops Control",
    age: "5h",
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
