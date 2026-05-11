"use client";

import { useTransition } from "react";
import { useRouter } from "next/navigation";
import { adminCopy } from "@/lib/admin-copy";
import { createJob, type ReportTemplate } from "@/lib/analytics-reporting";
import type { Role } from "@/lib/rbac";

interface ReportBuilderFormProps {
  template: ReportTemplate;
  viewerRole: Role;
}

/*
 * Phase 2 mock builder. The form is wired but does NOT call the
 * analytics-service yet — Phase 3 (TODO.md LG-9) will replace the
 * router.push() below with a server action that creates a real
 * report_job and redirects to its detail page. For now we redirect to
 * /analytics/jobs, where the deterministic snapshot already contains a
 * job with these same params, so reviewers can see the round-trip.
 *
 * SPEC-020 §3 reveal toggle: shown only to admin. Manager sees a note
 * but no checkbox, matching the "敏感欄位需二次授權" requirement.
 */
export function ReportBuilderForm({ template, viewerRole }: ReportBuilderFormProps) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();

  const isAdmin = viewerRole === "admin";

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    startTransition(() => {
      void submitReport(form);
    });
  }

  async function submitReport(form: HTMLFormElement) {
    const data = new FormData(form);
    const columns = data.getAll("columns").map(String);
    const startAt = normalizeDateTime(String(data.get("startAt") ?? ""));
    const endAt = normalizeDateTime(String(data.get("endAt") ?? ""));
    const job = await createJob({
      role: viewerRole,
      templateCode: template.code,
      params: {
        startAt,
        endAt,
        timezone: String(data.get("timezone") ?? "Asia/Taipei") as "Asia/Taipei" | "UTC",
        sortBy: String(data.get("sortBy") ?? template.fields[0]?.key ?? ""),
        sortDirection: String(data.get("sortDirection") ?? "asc") as "asc" | "desc",
        columns,
      },
      revealSensitiveFields: data.get("revealSensitiveFields") === "on",
    });
    router.push(`/analytics/jobs/${job.id}`);
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="space-y-6 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
    >
      <div className="grid gap-4 md:grid-cols-2">
        <Field id={`${template.code}-startAt`} label={adminCopy.analytics.fieldStartAt}>
          <input
            id={`${template.code}-startAt`}
            name="startAt"
            type="datetime-local"
            defaultValue="2026-05-10T00:00"
            className="min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[15px] tabular-nums tracking-[-0.224px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          />
        </Field>
        <Field id={`${template.code}-endAt`} label={adminCopy.analytics.fieldEndAt}>
          <input
            id={`${template.code}-endAt`}
            name="endAt"
            type="datetime-local"
            defaultValue="2026-05-10T23:59"
            className="min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[15px] tabular-nums tracking-[-0.224px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          />
        </Field>
      </div>

      <Field id={`${template.code}-timezone`} label={adminCopy.analytics.fieldTimezone}>
        <select
          id={`${template.code}-timezone`}
          name="timezone"
          defaultValue="Asia/Taipei"
          className="min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[15px] tracking-[-0.224px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
        >
          <option value="Asia/Taipei">{adminCopy.analytics.fieldTimezoneTaipei}</option>
          <option value="UTC">{adminCopy.analytics.fieldTimezoneUtc}</option>
        </select>
      </Field>

      <fieldset className="space-y-3">
        <legend className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
          {adminCopy.analytics.fieldColumns}
        </legend>
        <ul className="grid gap-2 sm:grid-cols-2">
          {template.fields.map((field) => (
            <li key={field.key}>
              <label className="flex min-h-11 items-center gap-3 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink">
                <input
                  type="checkbox"
                  name="columns"
                  value={field.key}
                  defaultChecked
                  className="h-4 w-4 accent-proph-primary"
                />
                <span className="flex flex-col">
                  <span>{field.label}</span>
                  <span className="font-mono text-[11px] tracking-[-0.12px] text-proph-ink-muted-48">
                    {field.key}
                    {field.sensitive ? " · sensitive" : ""}
                  </span>
                </span>
              </label>
            </li>
          ))}
        </ul>
      </fieldset>

      <div className="grid gap-4 md:grid-cols-[1fr_auto]">
        <Field id={`${template.code}-sortBy`} label={adminCopy.analytics.fieldSortBy}>
          <select
            id={`${template.code}-sortBy`}
            name="sortBy"
            defaultValue={template.fields[0]?.key}
            className="min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[15px] tracking-[-0.224px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          >
            {template.fields.map((field) => (
              <option key={field.key} value={field.key}>
                {field.label}
              </option>
            ))}
          </select>
        </Field>
        <Field id={`${template.code}-sortDirection`} label="方向">
          <select
            id={`${template.code}-sortDirection`}
            name="sortDirection"
            defaultValue="asc"
            className="min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[15px] tracking-[-0.224px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
          >
            <option value="asc">{adminCopy.analytics.fieldSortAsc}</option>
            <option value="desc">{adminCopy.analytics.fieldSortDesc}</option>
          </select>
        </Field>
      </div>

      {template.fields.some((field) => field.sensitive) && (
        <div className="rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4">
          {isAdmin ? (
            <label className="flex min-h-11 items-start gap-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
              <input type="checkbox" name="revealSensitiveFields" className="mt-1 h-4 w-4 accent-proph-primary" />
              <span>
                <span className="font-semibold">{adminCopy.analytics.fieldReveal}</span>
                <br />
                <span className="text-proph-ink-muted-80">{adminCopy.analytics.fieldRevealHelp}</span>
              </span>
            </label>
          ) : (
            <p className="text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.analytics.detailRevealLocked}
            </p>
          )}
        </div>
      )}

      <button
        type="submit"
        disabled={pending}
        className="inline-flex h-12 min-w-[200px] items-center justify-center rounded-proph-pill bg-proph-primary px-6 text-[15px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-60"
      >
        {adminCopy.analytics.submitCta}
      </button>
    </form>
  );
}

function normalizeDateTime(value: string): string {
  if (!value) return "2026-05-10T00:00:00+08:00";
  return value.length === 16 ? `${value}:00+08:00` : value;
}

function Field({ id, label, children }: { id: string; label: string; children: React.ReactNode }) {
  return (
    <label htmlFor={id} className="block space-y-2">
      <span className="block text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
        {label}
      </span>
      {children}
    </label>
  );
}
