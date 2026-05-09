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

const routes: OpsRoute[] = [
  {
    href: "/members",
    label: "Member management",
    domain: "member-management",
    summary: "Search, support context, account state review.",
    scope: "Read account profile, flags, linked cases, and escalation history.",
    owner: "Member Ops",
    openItems: 18,
    tone: "neutral",
  },
  {
    href: "/points",
    label: "Account points exceptions",
    domain: "points-exceptions",
    summary: "Triage balance disputes without direct ledger writes.",
    scope: "Open exception cases, attach evidence, and request ledger review.",
    owner: "Wallet Ops",
    openItems: 7,
    tone: "warning",
  },
  {
    href: "/affiliate",
    label: "Affiliate permissions & commission review",
    domain: "affiliate-review",
    summary: "Review affiliate access, payout holds, and commission anomalies.",
    scope: "Inspect permission state, commission queues, and required approvals.",
    owner: "Growth Ops",
    openItems: 11,
    tone: "neutral",
  },
  {
    href: "/kyc-risk",
    label: "KYC / risk events",
    domain: "kyc-risk-events",
    summary: "Monitor identity review, risk signals, and provider callbacks.",
    scope: "Coordinate risk event review; decisions remain audited and separated.",
    owner: "Risk Ops",
    openItems: 23,
    tone: "critical",
  },
  {
    href: "/compensation",
    label: "Compensation reviews",
    domain: "compensation-reviews",
    summary: "Assess customer-impact compensation requests.",
    scope: "Review incident linkage, prior credits, and approval requirements.",
    owner: "Support Leads",
    openItems: 5,
    tone: "success",
  },
  {
    href: "/approvals",
    label: "Manual approvals",
    domain: "manual-approvals",
    summary: "Central queue for sensitive operator approvals.",
    scope: "Route approvals to authorized reviewers; no live mutation from this stub.",
    owner: "Operations Control",
    openItems: 14,
    tone: "warning",
  },
];

const summaries: OpsQueueSummary[] = [
  { label: "Open queues", value: "78", detail: "All domains", tone: "neutral" },
  { label: "Risk escalations", value: "9", detail: "Needs same-day review", tone: "critical" },
  { label: "Approval ready", value: "16", detail: "Waiting on authorized operator", tone: "warning" },
  { label: "Resolved today", value: "31", detail: "Mock operational snapshot", tone: "success" },
];

const queue: OpsQueueItem[] = [
  {
    id: "OPS-1042",
    domain: "kyc-risk-events",
    title: "Tier 2 provider mismatch",
    detail: "Identity callback conflicts with profile country; risk event linked.",
    status: "new",
    assignee: "Risk triage",
    age: "24m",
  },
  {
    id: "OPS-1037",
    domain: "points-exceptions",
    title: "Points exception evidence complete",
    detail: "Support supplied transaction references; ledger review request ready.",
    status: "ready",
    assignee: "Wallet reviewer",
    age: "1h",
  },
  {
    id: "OPS-1029",
    domain: "affiliate-review",
    title: "Commission hold review",
    detail: "Affiliate permission change and payout hold need paired review.",
    status: "reviewing",
    assignee: "Growth ops",
    age: "3h",
  },
  {
    id: "OPS-1018",
    domain: "manual-approvals",
    title: "Manual approval policy check",
    detail: "Sensitive action blocked until two authorized reviewers sign off.",
    status: "blocked",
    assignee: "Ops control",
    age: "5h",
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
