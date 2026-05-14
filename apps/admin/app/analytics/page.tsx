import Link from "next/link";
import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchTemplates, type ReportTemplate } from "@/lib/analytics-reporting";
import { analyticsServerFetchOptions } from "@/lib/analytics-server-fetch";

const categoryLabel: Record<ReportTemplate["category"], string> = {
  read_only_operations: adminCopy.analytics.categoryOps,
  risk: adminCopy.analytics.categoryRisk,
  finance_review: adminCopy.analytics.categoryFinance,
};

export default function AnalyticsTemplatesPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        let templates: ReportTemplate[];
        try {
          templates = await fetchTemplates(await analyticsServerFetchOptions());
        } catch {
          return (
            <AdminShell
              eyebrow={adminCopy.analytics.eyebrow}
              title={adminCopy.analytics.listTitle}
              description={adminCopy.analytics.listDescription}
              role={viewer.role}
            >
              <AnalyticsUnavailableState />
            </AdminShell>
          );
        }

        return (
          <AdminShell
            eyebrow={adminCopy.analytics.eyebrow}
            title={adminCopy.analytics.listTitle}
            description={adminCopy.analytics.listDescription}
            role={viewer.role}
          >
            <div className="grid gap-4 lg:grid-cols-2">
              {templates.map((template) => (
                <TemplateCard key={template.code} template={template} />
              ))}
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function AnalyticsUnavailableState() {
  return (
    <section
      aria-labelledby="analytics-templates-unavailable-title"
      className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
    >
      <div className="flex flex-wrap items-center gap-2">
        <StatusPill tone="warning">
          {adminCopy.components.status.blocked}
        </StatusPill>
        <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-80">
          API_GATEWAY_URL
        </span>
      </div>
      <h2
        id="analytics-templates-unavailable-title"
        className="mt-3 text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
      >
        {adminCopy.analytics.templatesUnavailableTitle}
      </h2>
      <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {adminCopy.analytics.templatesUnavailableDescription}
      </p>
    </section>
  );
}

function TemplateCard({ template }: { template: ReportTemplate }) {
  const hasSensitive = template.fields.some((field) => field.sensitive);
  const retentionLabel =
    template.retention.category === "legal_audit"
      ? adminCopy.analytics.retentionLegal
      : adminCopy.analytics.retentionOps;

  return (
    <article className="flex flex-col gap-4 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-wrap items-center gap-2">
        <StatusPill
          tone={
            template.retention.category === "legal_audit"
              ? "warning"
              : "neutral"
          }
        >
          {categoryLabel[template.category]}
        </StatusPill>
        <span className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
          {template.schemaVersion}
        </span>
        {hasSensitive && (
          <span className="inline-flex items-center rounded-proph-pill border border-proph-feedback-warning bg-proph-canvas-parchment px-2 py-1 text-[11px] font-semibold leading-none tracking-[-0.12px] text-proph-ink">
            {adminCopy.analytics.sensitiveBadge}
          </span>
        )}
      </div>
      <header>
        <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
          {template.name}
        </h2>
        <p className="mt-2 font-mono text-[13px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          {template.code}
        </p>
      </header>
      <dl className="grid gap-3 text-[14px] leading-[1.43] tracking-[-0.224px]">
        <KeyValue
          label="輸出欄位"
          value={template.fields.map((field) => field.label).join("、")}
        />
        <KeyValue
          label="保留"
          value={`${retentionLabel}（到 ${template.retention.expiresAt.slice(0, 10)}）`}
        />
        <KeyValue label="最近一次預設快照" value={template.generatedAt} />
      </dl>
      <div className="mt-auto flex items-center justify-end">
        <Link
          href={`/analytics/run/${template.code}`}
          className="inline-flex min-h-11 items-center rounded-proph-pill bg-proph-primary px-5 text-[15px] leading-[1.29] tracking-[-0.224px] text-proph-on-primary transition-opacity hover:opacity-95 active:opacity-90"
        >
          {adminCopy.analytics.runCta}
        </Link>
      </div>
    </article>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd className="text-proph-ink">{value}</dd>
    </div>
  );
}
