import Link from "next/link";
import { notFound } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import {
  fetchJobDetail,
  type ReportFormat,
  type ReportJob,
} from "@/lib/analytics-reporting";
import type { Role } from "@/lib/rbac";

interface JobPageProps {
  params: Promise<{ id: string }>;
}

const formatLabel: Record<ReportFormat, string> = {
  csv: adminCopy.analytics.detailDownloadCsv,
  json: adminCopy.analytics.detailDownloadJson,
  xlsx: adminCopy.analytics.detailDownloadXlsx,
};

export default async function AnalyticsJobDetailPage({ params }: JobPageProps) {
  const { id } = await params;

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const detail = await fetchJobDetail(id);
        const job = detail.job;
        if (!job) {
          notFound();
        }

        const audit = detail.audit;

        return (
          <AdminShell
            eyebrow={adminCopy.analytics.eyebrow}
            title={`${adminCopy.analytics.detailTitle} · ${job.templateName}`}
            description={`${job.id} · ${job.version}`}
            role={viewer.role}
          >
            <div className="space-y-8">
              <Link
                href="/analytics/jobs"
                className="-mx-2 inline-flex min-h-11 items-center rounded-proph-sm px-2 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-primary transition-colors hover:bg-proph-canvas-parchment active:bg-proph-divider-soft"
              >
                ← {adminCopy.analytics.backToJobs}
              </Link>

              <ParamsCard job={job} />
              <RowsPreviewCard job={job} />
              <DownloadCard job={job} />
              <RevealCard job={job} viewerRole={viewer.role} />
              <RerunCard />
              <AuditCard audit={audit} />
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function ParamsCard({ job }: { job: ReportJob }) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailParamsHeading}
      </h2>
      <dl className="mt-4 grid gap-4 md:grid-cols-2">
        <KeyValue label={adminCopy.analytics.fieldStartAt} value={job.params.startAt} mono />
        <KeyValue label={adminCopy.analytics.fieldEndAt} value={job.params.endAt} mono />
        <KeyValue label={adminCopy.analytics.fieldTimezone} value={job.params.timezone} mono />
        <KeyValue
          label={adminCopy.analytics.fieldSortBy}
          value={`${job.params.sortBy} · ${job.params.sortDirection === "asc" ? adminCopy.analytics.fieldSortAsc : adminCopy.analytics.fieldSortDesc}`}
          mono
        />
        <KeyValue label="Schema" value={job.schemaVersion} mono />
        <KeyValue label="Version" value={job.version} mono />
        <KeyValue label="Generated at" value={job.generatedAt} mono />
        <KeyValue label="Row hash" value={job.rowHash} mono />
        <KeyValue label={adminCopy.analytics.jobOutputRowsColumn} value={String(job.outputRows)} />
        <KeyValue
          label={adminCopy.analytics.jobRetentionColumn}
          value={`${job.retention.category} · ${job.retention.expiresAt}`}
          mono
        />
      </dl>
      <div className="mt-5 flex flex-wrap gap-2">
        <span className="text-[12px] uppercase tracking-[0.08em] text-proph-ink-muted-48">
          {adminCopy.analytics.fieldColumns}
        </span>
        {job.params.columns.map((column) => (
          <span
            key={column}
            className="inline-flex items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-2 py-1 font-mono text-[11px] tracking-[-0.12px] text-proph-ink-muted-80"
          >
            {column}
          </span>
        ))}
      </div>
    </section>
  );
}

function RowsPreviewCard({ job }: { job: ReportJob }) {
  const columns = job.params.columns;
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailRowsHeading}
      </h2>
      <div className="mt-4 overflow-x-auto overscroll-x-contain rounded-proph-sm border border-proph-divider-soft">
        <table className="w-full border-collapse">
          <thead className="bg-proph-canvas-parchment">
            <tr>
              {columns.map((column) => (
                <th
                  key={column}
                  className="px-3 py-2 text-left font-mono text-[11px] font-semibold uppercase tracking-[-0.12px] text-proph-ink-muted-48"
                >
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {job.rows.map((row, rowIndex) => (
              <tr key={rowIndex} className="border-t border-proph-divider-soft">
                {columns.map((column) => (
                  <td
                    key={column}
                    className="px-3 py-2 font-mono text-[13px] tabular-nums tracking-[-0.224px] text-proph-ink"
                  >
                    {String(row[column] ?? "")}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function DownloadCard({ job }: { job: ReportJob }) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailDownloadHeading}
      </h2>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        {job.availableFormats.map((format) => (
          <a
            key={format}
            href={`/api/analytics/jobs/${job.id}/export?format=${format}`}
            download
            className="inline-flex min-h-12 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[15px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary transition-opacity hover:opacity-95 active:opacity-90"
          >
            {formatLabel[format]}
          </a>
        ))}
      </div>
    </section>
  );
}

function RevealCard({ job, viewerRole }: { job: ReportJob; viewerRole: Role }) {
  const hasSensitive = job.params.columns.some((column) => column.endsWith("_email") || column.endsWith("_id"));
  if (!hasSensitive) return null;
  const unlocked = job.rows.some((row) => {
    const email = row["member_email"];
    return typeof email === "string" && email.includes("@") && !email.includes("***");
  });
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailRevealHeading}
      </h2>
      <p className="mt-3 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
        {unlocked ? adminCopy.analytics.detailRevealUnlocked : adminCopy.analytics.detailRevealLocked}
      </p>
      {viewerRole === "admin" && !unlocked && (
        <p className="mt-3 text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          Phase 3 將在此處嵌入 MFA step-up 對話框（SPEC-010 §9.3.3）。
        </p>
      )}
    </section>
  );
}

function RerunCard() {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailRerunHeading}
      </h2>
      <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {adminCopy.analytics.detailRerunBody}
      </p>
      <button
        type="button"
        disabled
        title="Phase 3 串接 analytics-service 後啟用。"
        className="mt-4 inline-flex h-11 items-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary opacity-50"
      >
        {adminCopy.analytics.detailRerunCta}
      </button>
    </section>
  );
}

function AuditCard({ audit }: { audit: { action: string; actorId: string; actionAt: string; details: Record<string, string> }[] }) {
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        {adminCopy.analytics.detailAuditHeading}
      </h2>
      {audit.length === 0 ? (
        <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          {adminCopy.analytics.detailAuditEmpty}
        </p>
      ) : (
        <ul className="mt-4 divide-y divide-proph-hairline">
          {audit.map((record, index) => (
            <li key={index} className="grid gap-2 py-3 md:grid-cols-[1fr_auto]">
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">
                  {record.action}
                </span>
                <span className="text-[14px] tracking-[-0.224px] text-proph-ink">
                  {Object.entries(record.details)
                    .map(([key, value]) => `${key}=${value}`)
                    .join(" · ")}
                </span>
              </div>
              <div className="text-left text-[13px] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                <p className="font-mono">{record.actorId}</p>
                <p className="font-mono">{record.actionAt}</p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function KeyValue({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">{label}</dt>
      <dd
        className={
          mono
            ? "font-mono text-[14px] tracking-[-0.224px] text-proph-ink"
            : "text-[14px] tracking-[-0.224px] text-proph-ink"
        }
      >
        {value}
      </dd>
    </div>
  );
}
