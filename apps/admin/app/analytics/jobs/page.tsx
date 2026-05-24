import Link from "next/link";
import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchJobs } from "@/lib/analytics-reporting";
import { analyticsServerFetchOptions } from "@/lib/analytics-server-fetch";

export default function AnalyticsJobsPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {async () => {
        const reports = await fetchJobs(
          undefined,
          await analyticsServerFetchOptions(),
        );
        return (
          <AdminShell
            eyebrow={adminCopy.analytics.eyebrow}
            title={adminCopy.analytics.jobsTitle}
            description={adminCopy.analytics.jobsDescription}
          >
            <section
              aria-labelledby="analytics-jobs-title"
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas"
            >
              <header className="px-5 pt-5 md:px-6 md:pt-6">
                <h2
                  id="analytics-jobs-title"
                  className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
                >
                  {adminCopy.analytics.jobsTitle}
                </h2>
                <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {adminCopy.analytics.jobsDescription}
                </p>
              </header>
              <ul className="mt-4 divide-y divide-proph-hairline">
                {reports.map((job) => {
                  const retentionTone =
                    job.retention.category === "legal_audit"
                      ? "warning"
                      : "neutral";
                  return (
                    <li key={job.id}>
                      <Link
                        href={`/analytics/jobs/${job.id}`}
                        className="grid min-h-[88px] gap-3 px-5 py-5 transition-colors hover:bg-proph-surface-pearl active:bg-proph-canvas-parchment md:grid-cols-[180px_1fr_auto] md:px-6"
                      >
                        <div className="flex flex-col gap-1">
                          <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">
                            {job.id}
                          </span>
                          <span className="text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">
                            {job.templateName}
                          </span>
                          <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                            {job.templateCode}
                          </span>
                        </div>

                        <div className="flex flex-wrap items-start gap-x-4 gap-y-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                          <Cell
                            label={adminCopy.analytics.jobVersionColumn}
                            value={job.version}
                            mono
                          />
                          <Cell
                            label={adminCopy.analytics.jobGeneratedAtColumn}
                            value={job.generatedAt}
                            mono
                          />
                          <Cell
                            label={adminCopy.analytics.jobOutputRowsColumn}
                            value={String(job.outputRows)}
                          />
                          <Cell
                            label={adminCopy.analytics.jobRowHashColumn}
                            value={shortenHash(job.rowHash)}
                            mono
                          />
                        </div>

                        <div className="flex flex-col items-start gap-2 md:items-end">
                          <StatusPill tone={retentionTone}>
                            {job.retention.category === "legal_audit"
                              ? adminCopy.analytics.retentionLegal
                              : adminCopy.analytics.retentionOps}
                          </StatusPill>
                          <span className="flex flex-wrap gap-1">
                            {job.availableFormats.map((format) => (
                              <span
                                key={format}
                                className="inline-flex items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-2 py-1 font-mono text-[11px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-80"
                              >
                                {format}
                              </span>
                            ))}
                          </span>
                        </div>
                      </Link>
                    </li>
                  );
                })}
              </ul>
            </section>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function Cell({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <span className="inline-flex flex-col">
      <span className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">
        {label}
      </span>
      <span className={mono ? "font-mono text-proph-ink" : "text-proph-ink"}>
        {value}
      </span>
    </span>
  );
}

function shortenHash(hash: string): string {
  if (hash.length < 22) return hash;
  return `${hash.slice(0, 14)}…${hash.slice(-6)}`;
}
