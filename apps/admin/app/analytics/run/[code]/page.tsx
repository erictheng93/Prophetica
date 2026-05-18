import Link from "next/link";
import { notFound } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { ReportBuilderForm } from "@/components/report-builder-form";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchTemplates } from "@/lib/analytics-reporting";
import { analyticsServerFetchOptions } from "@/lib/analytics-server-fetch";

interface RunPageProps {
  params: Promise<{ code: string }>;
}

export default async function AnalyticsRunPage({ params }: RunPageProps) {
  const { code } = await params;

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const template = (
          await fetchTemplates(await analyticsServerFetchOptions())
        ).find((row) => row.code === code);
        if (!template) {
          notFound();
        }

        const retentionLabel =
          template.retention.category === "legal_audit"
            ? adminCopy.analytics.retentionLegal
            : adminCopy.analytics.retentionOps;

        return (
          <AdminShell
            eyebrow={adminCopy.analytics.eyebrow}
            title={`${adminCopy.analytics.builderTitle} · ${template.name}`}
            description={adminCopy.analytics.builderDescription}
          >
            <div className="space-y-6">
              <Link
                href="/analytics"
                className="-mx-2 inline-flex min-h-11 items-center rounded-proph-sm px-2 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-primary transition-colors hover:bg-proph-canvas-parchment active:bg-proph-divider-soft"
              >
                ← {adminCopy.analytics.backToList}
              </Link>

              <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusPill
                    tone={
                      template.retention.category === "legal_audit"
                        ? "warning"
                        : "neutral"
                    }
                  >
                    {template.schemaVersion}
                  </StatusPill>
                  <span className="font-mono text-[12px] tracking-[-0.12px] text-proph-ink-muted-80">
                    {template.code}
                  </span>
                  <span className="text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                    {retentionLabel}
                  </span>
                </div>
                <h2 className="mt-3 text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
                  {template.name}
                </h2>
                <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {adminCopy.analytics.builderDescription}
                </p>
              </section>

              <ReportBuilderForm template={template} viewerRole={viewer.role} />
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}
