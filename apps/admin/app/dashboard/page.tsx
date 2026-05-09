import { AdminShell } from "@/components/admin-shell";
import { StatusPill } from "@/components/status-pill";
import { getDecisionSupportSnapshot } from "@/lib/ops-facade";

export default function DecisionSupportDashboardPage() {
  const snapshot = getDecisionSupportSnapshot();

  return (
    <AdminShell
      eyebrow="decision-support"
      title="Decision support dashboard"
      description="Read-only KPI, alerts, versioned reports, and audit-trail lookup for operators after the operations entry surface has routed work."
    >
      <div className="space-y-8">
        <section aria-labelledby="dashboard-kpi-title">
          <div className="mb-3">
            <h2 id="dashboard-kpi-title" className="text-[22px] font-semibold leading-7">
              kpi and alerts
            </h2>
            <p className="mt-1 text-[14px] leading-5 text-proph-ink-muted-80">
              Observation only. Operational changes remain in the dedicated workspaces.
            </p>
          </div>
          <div className="grid gap-3 md:grid-cols-4">
            {snapshot.metrics.map((metric) => (
              <article
                className="rounded-proph-sm border border-proph-hairline bg-proph-canvas p-4"
                key={metric.id}
              >
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-[14px] font-semibold leading-5">{metric.label}</h3>
                  <StatusPill tone={metric.tone}>{metric.id}</StatusPill>
                </div>
                <p className="mt-4 text-[30px] font-semibold leading-none">{metric.value}</p>
                <p className="mt-2 text-[13px] leading-5 text-proph-ink-muted-80">{metric.detail}</p>
              </article>
            ))}
          </div>
        </section>

        <section aria-labelledby="versioned-reports-title">
          <div className="mb-3">
            <h2 id="versioned-reports-title" className="text-[22px] font-semibold leading-7">
              versioned-reports
            </h2>
            <p className="mt-1 text-[14px] leading-5 text-proph-ink-muted-80">
              Report snapshots are versioned, hash-addressed, and owned by an accountable role.
            </p>
          </div>
          <div className="overflow-hidden rounded-proph-sm border border-proph-hairline">
            {snapshot.reports.map((report) => (
              <article
                className="grid gap-3 border-b border-proph-hairline bg-proph-canvas p-4 last:border-b-0 md:grid-cols-[1fr_auto]"
                key={report.reportId}
              >
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[12px] font-semibold text-proph-ink-muted-80">{report.reportId}</span>
                    <StatusPill>{report.version}</StatusPill>
                  </div>
                  <h3 className="mt-2 text-[17px] font-semibold leading-6">{report.title}</h3>
                  <p className="mt-1 font-mono text-[13px] leading-5 text-proph-ink-muted-80">{report.hash}</p>
                </div>
                <div className="text-left text-[13px] leading-5 text-proph-ink-muted-80 md:text-right">
                  <p>{report.owner}</p>
                  <p>{report.generatedAt}</p>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section aria-labelledby="audit-trail-title">
          <div className="mb-3">
            <h2 id="audit-trail-title" className="text-[22px] font-semibold leading-7">
              audit-trail
            </h2>
            <p className="mt-1 text-[14px] leading-5 text-proph-ink-muted-80">
              Saved query families for evidence lookup. This page does not create approvals or ledger writes.
            </p>
          </div>
          <ul className="grid gap-3 md:grid-cols-2">
            {snapshot.auditQueries.map((query) => (
              <li className="rounded-proph-sm border border-proph-hairline bg-proph-canvas p-4" key={query}>
                <span className="font-mono text-[13px] leading-5 text-proph-ink-muted-80">{query}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </AdminShell>
  );
}
