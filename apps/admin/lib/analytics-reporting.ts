import type { Role } from "./rbac.js";

export type ReportFormat = "csv" | "json" | "xlsx";
export type ReportTimezone = "Asia/Taipei" | "UTC";
export type ReportSortDirection = "asc" | "desc";
export type RetentionCategory = "operations" | "legal_audit";
export type ReportCell = string | number | boolean | null;
export type ReportRow = Record<string, ReportCell>;

export interface ReportFieldDefinition {
  key: string;
  label: string;
  sensitive?: boolean;
}

export interface ReportRetentionPolicy {
  category: RetentionCategory;
  expiresAt: string;
}

export interface ReportScheduleHint {
  templateCode: string;
  templateName?: string;
  cron: string;
  nextFireHint: string;
  owner: string;
  retention?: RetentionCategory;
  status?: "enabled" | "paused";
}

export interface ReportTemplate {
  code: string;
  name: string;
  category: "read_only_operations" | "risk" | "finance_review";
  schemaVersion: "analytics.v1";
  viewerRoles: readonly Role[];
  fields: readonly ReportFieldDefinition[];
  generatedAt: string;
  retention: ReportRetentionPolicy;
  rows: readonly ReportRow[];
  schedule?: Omit<ReportScheduleHint, "templateCode" | "templateName" | "retention" | "status">;
}

export interface ReportParams {
  startAt: string;
  endAt: string;
  timezone: ReportTimezone;
  sortBy: string;
  sortDirection: ReportSortDirection;
  columns: readonly string[];
}

export interface CreateReportJobRequest {
  actorId?: string;
  role?: Role;
  templateCode: string;
  params: ReportParams;
  revealSensitiveFields?: boolean;
  secondAuthorizationId?: string;
  rerunOfJobId?: string;
}

export interface SourceProvenance {
  sourceService: string;
  sourceVersion: string;
  cutoffAt: string;
  sourceGeneratedAt: string;
  correlationId: string;
}

export interface SourceSnapshot extends SourceProvenance {
  sources: SourceProvenance[];
}

export interface ReportJob {
  id: string;
  templateCode: string;
  templateName: string;
  requestedBy: string;
  schemaVersion: "analytics.v1";
  version: string;
  state: "completed";
  generatedAt: string;
  params: ReportParams;
  sourceSnapshot: SourceSnapshot;
  outputRows: number;
  rowHash: string;
  availableFormats: ReportFormat[];
  retention: ReportRetentionPolicy;
  rows: ReportRow[];
}

export interface ReportAuditRecord {
  actorId: string;
  action:
    | "report_job_created"
    | "report_job_rerun"
    | "sensitive_fields_revealed"
    | "sensitive_fields_revealed_denied"
    | "report_export_deleted";
  jobId: string;
  actionAt: string;
  details: Record<string, string>;
}

export interface RecordExportDeletionRequest {
  actorId?: string;
  jobId?: string;
  format: ReportFormat;
  reason: string;
}

export interface ReportExportPayload {
  filename: string;
  contentType: string;
  body: Uint8Array;
  bodyText: () => string;
}

export interface AnalyticsReportingMetric {
  id: "kpi" | "alerts" | "versioned-reports" | "audit-trail";
  label: string;
  value: string;
  detail: string;
  tone: "neutral" | "warning" | "success" | "critical";
}

export interface AnalyticsReportingSnapshot {
  generatedAt: string;
  metrics: AnalyticsReportingMetric[];
  reports: ReportJob[];
  auditRecords: ReportAuditRecord[];
}

const BFF_BASE = "/api/analytics";

async function jsonOrThrow(response: Response): Promise<unknown> {
  if (!response.ok) {
    throw new Error(`analytics bff ${response.status}`);
  }
  if (response.status === 204) {
    return null;
  }
  return response.json();
}

export async function fetchTemplates(): Promise<ReportTemplate[]> {
  const response = await fetch(`${BFF_BASE}/templates`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportTemplate[];
}

export async function createJob(req: CreateReportJobRequest): Promise<ReportJob> {
  const response = await fetch(`${BFF_BASE}/jobs`, {
    method: "POST",
    cache: "no-store",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  const payload = (await jsonOrThrow(response)) as ReportJob | { job: ReportJob };
  return "job" in payload ? payload.job : payload;
}

export async function fetchJobs(filter?: { templateCode?: string }): Promise<ReportJob[]> {
  const query = new URLSearchParams();
  if (filter?.templateCode) query.set("template_code", filter.templateCode);
  const suffix = query.size > 0 ? `?${query.toString()}` : "";
  const response = await fetch(`${BFF_BASE}/jobs${suffix}`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportJob[];
}

export async function fetchJobDetail(id: string): Promise<{ job: ReportJob; audit: ReportAuditRecord[] }> {
  const response = await fetch(`${BFF_BASE}/jobs/${encodeURIComponent(id)}`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as { job: ReportJob; audit: ReportAuditRecord[] };
}

export async function exportJob(id: string, format: ReportFormat): Promise<Response> {
  return fetch(`${BFF_BASE}/jobs/${encodeURIComponent(id)}/export?format=${encodeURIComponent(format)}`, {
    cache: "no-store",
  });
}

export async function recordExportDeletion(id: string, req: RecordExportDeletionRequest): Promise<void> {
  const response = await fetch(`${BFF_BASE}/jobs/${encodeURIComponent(id)}/deletions`, {
    method: "POST",
    cache: "no-store",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  await jsonOrThrow(response);
}

export async function fetchSchedules(): Promise<ReportScheduleHint[]> {
  const response = await fetch(`${BFF_BASE}/schedules`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportScheduleHint[];
}

export async function fetchDashboardMetrics(): Promise<AnalyticsReportingMetric[]> {
  const response = await fetch(`${BFF_BASE}/dashboard-metrics`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as AnalyticsReportingMetric[];
}

export async function fetchLatestJobsPerTemplate(): Promise<ReportJob[]> {
  const response = await fetch(`${BFF_BASE}/jobs/latest-per-template`, { cache: "no-store" });
  return (await jsonOrThrow(response)) as ReportJob[];
}
