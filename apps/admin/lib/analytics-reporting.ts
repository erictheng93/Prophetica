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
  schedule?: Omit<
    ReportScheduleHint,
    "templateCode" | "templateName" | "retention" | "status"
  >;
}

export interface ReportParams {
  startAt: string;
  endAt: string;
  timezone: ReportTimezone;
  sortBy: string;
  sortDirection: ReportSortDirection;
  columns: readonly string[];
}

export function timezonesForRole(role: Role): readonly ReportTimezone[] {
  return role === "admin" ? ["Asia/Taipei", "UTC"] : ["Asia/Taipei"];
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

export interface AnalyticsFetchOptions {
  baseUrl?: string;
  headers?: HeadersInit;
}

function bffUrl(path: string, options?: AnalyticsFetchOptions): string {
  const relativePath = `${BFF_BASE}${path}`;
  return options?.baseUrl
    ? new URL(relativePath, options.baseUrl).toString()
    : relativePath;
}

function headersFor(
  options?: AnalyticsFetchOptions,
  extra?: Record<string, string>,
): Record<string, string> | undefined {
  const headers: Record<string, string> = {};

  if (options?.headers instanceof Headers) {
    for (const [key, value] of options.headers) {
      headers[key] = value;
    }
  } else if (Array.isArray(options?.headers)) {
    for (const [key, value] of options.headers) {
      headers[key] = value;
    }
  } else if (options?.headers) {
    Object.assign(headers, options.headers);
  }

  if (extra) {
    Object.assign(headers, extra);
  }

  return Object.keys(headers).length > 0 ? headers : undefined;
}

async function jsonOrThrow(response: Response): Promise<unknown> {
  if (!response.ok) {
    throw new Error(`analytics bff ${response.status}`);
  }
  if (response.status === 204) {
    return null;
  }
  return response.json();
}

export async function fetchTemplates(
  options?: AnalyticsFetchOptions,
): Promise<ReportTemplate[]> {
  const response = await fetch(bffUrl("/templates", options), {
    cache: "no-store",
    headers: headersFor(options),
  });
  return (await jsonOrThrow(response)) as ReportTemplate[];
}

export async function createJob(
  req: CreateReportJobRequest,
  options?: AnalyticsFetchOptions,
): Promise<ReportJob> {
  const response = await fetch(bffUrl("/jobs", options), {
    method: "POST",
    cache: "no-store",
    headers: headersFor(options, { "content-type": "application/json" }),
    body: JSON.stringify(req),
  });
  const payload = (await jsonOrThrow(response)) as
    | ReportJob
    | { job: ReportJob };
  return "job" in payload ? payload.job : payload;
}

export async function fetchJobs(
  filter?: {
    templateCode?: string;
  },
  options?: AnalyticsFetchOptions,
): Promise<ReportJob[]> {
  const query = new URLSearchParams();
  if (filter?.templateCode) query.set("template_code", filter.templateCode);
  const suffix = query.size > 0 ? `?${query.toString()}` : "";
  const response = await fetch(bffUrl(`/jobs${suffix}`, options), {
    cache: "no-store",
    headers: headersFor(options),
  });
  return (await jsonOrThrow(response)) as ReportJob[];
}

export async function fetchJobDetail(
  id: string,
  options?: AnalyticsFetchOptions,
): Promise<{ job: ReportJob; audit: ReportAuditRecord[] }> {
  const response = await fetch(
    bffUrl(`/jobs/${encodeURIComponent(id)}`, options),
    {
      cache: "no-store",
      headers: headersFor(options),
    },
  );
  return (await jsonOrThrow(response)) as {
    job: ReportJob;
    audit: ReportAuditRecord[];
  };
}

export async function exportJob(
  id: string,
  format: ReportFormat,
  options?: AnalyticsFetchOptions,
): Promise<Response> {
  return fetch(
    bffUrl(
      `/jobs/${encodeURIComponent(id)}/export?format=${encodeURIComponent(format)}`,
      options,
    ),
    {
      cache: "no-store",
      headers: headersFor(options),
    },
  );
}

export async function recordExportDeletion(
  id: string,
  req: RecordExportDeletionRequest,
  options?: AnalyticsFetchOptions,
): Promise<void> {
  const response = await fetch(
    bffUrl(`/jobs/${encodeURIComponent(id)}/deletions`, options),
    {
      method: "POST",
      cache: "no-store",
      headers: headersFor(options, { "content-type": "application/json" }),
      body: JSON.stringify(req),
    },
  );
  await jsonOrThrow(response);
}

export async function fetchSchedules(
  options?: AnalyticsFetchOptions,
): Promise<ReportScheduleHint[]> {
  const response = await fetch(bffUrl("/schedules", options), {
    cache: "no-store",
    headers: headersFor(options),
  });
  return (await jsonOrThrow(response)) as ReportScheduleHint[];
}

export async function fetchDashboardMetrics(
  options?: AnalyticsFetchOptions,
): Promise<AnalyticsReportingMetric[]> {
  const response = await fetch(bffUrl("/dashboard-metrics", options), {
    cache: "no-store",
    headers: headersFor(options),
  });
  return (await jsonOrThrow(response)) as AnalyticsReportingMetric[];
}

export async function fetchLatestJobsPerTemplate(
  options?: AnalyticsFetchOptions,
): Promise<ReportJob[]> {
  const response = await fetch(bffUrl("/jobs/latest-per-template", options), {
    cache: "no-store",
    headers: headersFor(options),
  });
  return (await jsonOrThrow(response)) as ReportJob[];
}
