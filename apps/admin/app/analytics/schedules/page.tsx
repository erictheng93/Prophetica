import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";

interface ScheduleRow {
  id: string;
  templateCode: string;
  templateName: string;
  cron: string;
  nextFireAt: string;
  owner: string;
  status: "enabled" | "paused";
  retention: "operations" | "legal_audit";
}

const placeholderSchedules: ScheduleRow[] = [
  {
    id: "sched-001",
    templateCode: "ops-kpi-daily",
    templateName: "營運 KPI 日報",
    cron: "0 8 * * *",
    nextFireAt: "2026-05-11T08:00:00+08:00",
    owner: "Ops Control",
    status: "enabled",
    retention: "operations",
  },
  {
    id: "sched-002",
    templateCode: "risk-alert-summary",
    templateName: "風控警示摘要",
    cron: "0 9 * * MON-FRI",
    nextFireAt: "2026-05-12T09:00:00+08:00",
    owner: "Risk Ops",
    status: "enabled",
    retention: "operations",
  },
  {
    id: "sched-003",
    templateCode: "finance-reconciliation",
    templateName: "財務對帳快照",
    cron: "0 1 * * *",
    nextFireAt: "2026-05-11T01:00:00+08:00",
    owner: "Compliance",
    status: "enabled",
    retention: "legal_audit",
  },
  {
    id: "sched-004",
    templateCode: "kyc-compliance-snapshot",
    templateName: "KYC 合規快照",
    cron: "0 10 1 * *",
    nextFireAt: "2026-06-01T10:00:00+08:00",
    owner: "Compliance",
    status: "paused",
    retention: "legal_audit",
  },
];

export default function AnalyticsSchedulesPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          eyebrow={adminCopy.analytics.eyebrow}
          title={adminCopy.analytics.schedulesTitle}
          description={adminCopy.analytics.schedulesDescription}
          role={viewer.role}
        >
          <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas">
            <header className="px-5 pt-5 md:px-6 md:pt-6">
              <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                {adminCopy.analytics.schedulesTitle}
              </h2>
              <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {adminCopy.analytics.schedulesDescription}
              </p>
            </header>
            <ul className="mt-4 divide-y divide-proph-hairline">
              {placeholderSchedules.map((row) => (
                <li key={row.id}>
                  <article className="grid min-h-[88px] gap-3 px-5 py-5 md:grid-cols-[180px_1fr_auto] md:px-6">
                    <div className="flex flex-col gap-1">
                      <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">{row.id}</span>
                      <span className="text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">
                        {row.templateName}
                      </span>
                      <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                        {row.templateCode}
                      </span>
                    </div>
                    <div className="flex flex-wrap items-start gap-x-4 gap-y-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                      <Cell label={adminCopy.analytics.schedulesCron} value={row.cron} mono />
                      <Cell label={adminCopy.analytics.schedulesNextFire} value={row.nextFireAt} mono />
                      <Cell label={adminCopy.analytics.schedulesOwner} value={row.owner} />
                      <Cell
                        label={adminCopy.analytics.jobRetentionColumn}
                        value={
                          row.retention === "legal_audit"
                            ? adminCopy.analytics.retentionLegal
                            : adminCopy.analytics.retentionOps
                        }
                      />
                    </div>
                    <div className="flex flex-col items-start gap-2 md:items-end">
                      <StatusPill tone={row.status === "enabled" ? "success" : "warning"}>
                        {row.status === "enabled"
                          ? adminCopy.analytics.schedulesStatusEnabled
                          : adminCopy.analytics.schedulesStatusPaused}
                      </StatusPill>
                      <button
                        type="button"
                        disabled
                        title={adminCopy.analytics.schedulesActionDisabled}
                        className="inline-flex h-11 items-center rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 opacity-60"
                      >
                        編輯排程
                      </button>
                    </div>
                  </article>
                </li>
              ))}
            </ul>
            <p className="border-t border-proph-hairline px-5 py-4 text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48 md:px-6">
              {adminCopy.analytics.schedulesActionDisabled}
            </p>
          </section>
        </AdminShell>
      )}
    </RoleGate>
  );
}

function Cell({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <span className="inline-flex flex-col">
      <span className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">{label}</span>
      <span className={mono ? "font-mono text-proph-ink" : "text-proph-ink"}>{value}</span>
    </span>
  );
}
