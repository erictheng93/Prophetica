import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { getDecisionSupportSnapshot } from "@/lib/ops-facade";

export default function DecisionSupportDashboardPage() {
  const snapshot = getDecisionSupportSnapshot();

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          eyebrow={adminCopy.dashboard.eyebrow}
          title={adminCopy.dashboard.title}
          description={adminCopy.dashboard.description}
          role={viewer.role}
        >
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
          <div className="grid gap-4 md:grid-cols-4">
            {snapshot.metrics.map((metric) => (
              <article
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
                key={metric.id}
              >
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">{metric.label}</h3>
                  <StatusPill tone={metric.tone}>{adminCopy.components.previewSnapshot}</StatusPill>
                </div>
                <p className="mt-4 text-[34px] font-semibold leading-[1.07] tracking-[-0.374px]">{metric.value}</p>
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
            {snapshot.reports.map((report) => (
              <article
                className="grid gap-3 border-b border-proph-hairline bg-proph-canvas p-5 last:border-b-0 md:grid-cols-[1fr_auto]"
                key={report.reportId}
              >
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80">{report.reportId}</span>
                    <StatusPill>{report.version}</StatusPill>
                  </div>
                  <h3 className="mt-2 text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">{report.title}</h3>
                  <p className="mt-1 font-mono text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{report.hash}</p>
                </div>
                <div className="text-left text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                  <p>{report.owner}</p>
                  <p>{report.generatedAt}</p>
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
            {snapshot.auditQueries.map((query) => (
              <li className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5" key={query}>
                <span className="font-mono text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{query}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
