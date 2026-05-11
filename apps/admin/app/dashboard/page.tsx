import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchDashboardMetrics, fetchLatestJobsPerTemplate } from "@/lib/analytics-reporting";

export default function DecisionSupportDashboardPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          eyebrow={adminCopy.dashboard.eyebrow}
          title={adminCopy.dashboard.title}
          description={adminCopy.dashboard.description}
          role={viewer.role}
        >
          <AnalyticsReportingContent />
        </AdminShell>
      )}
    </RoleGate>
  );
}

async function AnalyticsReportingContent() {
  const [metrics, reports] = await Promise.all([fetchDashboardMetrics(), fetchLatestJobsPerTemplate()]);

  return (
      <div className="space-y-8">
        <section aria-labelledby="dashboard-kpi-title" data-insight="decision-support kpi alerts">
          <div className="mb-4">
            <h2 id="dashboard-kpi-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.dashboard.kpiHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.dashboard.kpiDescription}
            </p>
          </div>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {metrics.map((metric) => (
              <article
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
                key={metric.id}
              >
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">{metric.label}</h3>
                  <StatusPill tone={metric.tone}>{adminCopy.components.previewSnapshot}</StatusPill>
                </div>
                <p className="mt-4 text-[28px] font-semibold leading-[1.07] tracking-[-0.374px] sm:text-[34px]">{metric.value}</p>
                <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{metric.detail}</p>
              </article>
            ))}
          </div>
        </section>

        <section aria-labelledby="versioned-reports-title" data-insight="versioned-reports">
          <div className="mb-4">
            <h2 id="versioned-reports-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.dashboard.reportsHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.dashboard.reportsDescription}
            </p>
          </div>
          <div className="overflow-hidden rounded-proph-lg border border-proph-hairline">
            {reports.map((report) => (
              <article
                className="grid gap-3 border-b border-proph-hairline bg-proph-canvas p-5 last:border-b-0 md:grid-cols-[1fr_auto]"
                key={report.id}
              >
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80">{report.id}</span>
                    <StatusPill>{report.version}</StatusPill>
                  </div>
                  <h3 className="mt-2 text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">{report.templateName}</h3>
                  <p className="mt-1 font-mono text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{report.rowHash}</p>
                  <p className="mt-2 text-[13px] leading-[1.38] tracking-[-0.12px] text-proph-ink-muted-80">
                    {report.availableFormats.join(" / ")} · {report.outputRows} rows
                  </p>
                </div>
                <div className="text-left text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                  <p>{report.retention.category}</p>
                  <p>{report.generatedAt}</p>
                  <p>{report.retention.expiresAt}</p>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section aria-labelledby="audit-trail-title" data-insight="audit-trail">
          <div className="mb-4">
            <h2 id="audit-trail-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.dashboard.auditHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.dashboard.auditDescription}
            </p>
          </div>
          <ul className="grid gap-4 md:grid-cols-2">
            {reports.length === 0 ? (
              <li className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                No report jobs have been generated yet.
              </li>
            ) : reports.map((report) => (
              <li className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5" key={report.id}>
                <span className="font-mono text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{report.sourceSnapshot.sourceVersion}</span>
                <p className="mt-2 text-[13px] leading-[1.38] tracking-[-0.12px] text-proph-ink-muted-80">
                  {report.sourceSnapshot.sourceService} · {report.id} · {report.sourceSnapshot.sourceGeneratedAt}
                </p>
              </li>
            ))}
          </ul>
        </section>
      </div>
  );
}
