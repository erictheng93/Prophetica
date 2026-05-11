import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchSchedules } from "@/lib/analytics-reporting";

export default function AnalyticsSchedulesPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const schedules = await fetchSchedules();
        return (
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
                {schedules.map((row, index) => {
                  const status = row.status ?? "enabled";
                  const retention = row.retention ?? "operations";
                  return (
                    <li key={row.templateCode}>
                      <article className="grid min-h-[88px] gap-3 px-5 py-5 md:grid-cols-[180px_1fr_auto] md:px-6">
                        <div className="flex flex-col gap-1">
                          <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">
                            sched-{String(index + 1).padStart(3, "0")}
                          </span>
                          <span className="text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">
                            {row.templateName ?? row.templateCode}
                          </span>
                          <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                            {row.templateCode}
                          </span>
                        </div>
                        <div className="flex flex-wrap items-start gap-x-4 gap-y-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                          <Cell label={adminCopy.analytics.schedulesCron} value={row.cron} mono />
                          <Cell label={adminCopy.analytics.schedulesNextFire} value={row.nextFireHint} mono />
                          <Cell label={adminCopy.analytics.schedulesOwner} value={row.owner} />
                          <Cell
                            label={adminCopy.analytics.jobRetentionColumn}
                            value={
                              retention === "legal_audit"
                                ? adminCopy.analytics.retentionLegal
                                : adminCopy.analytics.retentionOps
                            }
                          />
                        </div>
                        <div className="flex flex-col items-start gap-2 md:items-end">
                          <StatusPill tone={status === "enabled" ? "success" : "warning"}>
                            {status === "enabled"
                              ? adminCopy.analytics.schedulesStatusEnabled
                              : adminCopy.analytics.schedulesStatusPaused}
                          </StatusPill>
                          <button
                            type="button"
                            disabled
                            title={adminCopy.analytics.schedulesActionDisabled}
                            className="inline-flex h-11 items-center rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 opacity-60"
                          >
                            Disabled
                          </button>
                        </div>
                      </article>
                    </li>
                  );
                })}
              </ul>
              <p className="border-t border-proph-hairline px-5 py-4 text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48 md:px-6">
                {adminCopy.analytics.schedulesActionDisabled}
              </p>
            </section>
          </AdminShell>
        );
      }}
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
