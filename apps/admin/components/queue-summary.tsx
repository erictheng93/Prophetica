import type { OpsQueueSummary } from "@/lib/ops-facade";
import { adminCopy } from "@/lib/admin-copy";
import { StatusPill } from "./status-pill";

interface QueueSummaryProps {
  summaries: OpsQueueSummary[];
}

export function QueueSummary({ summaries }: QueueSummaryProps) {
  return (
    <section aria-label="Queue summary" className="grid gap-3 md:grid-cols-4">
      {summaries.map((summary) => (
        <article className="rounded-proph-sm border border-proph-hairline bg-proph-canvas p-4" key={summary.label}>
          <div className="flex items-center justify-between gap-3">
            <h2 className="text-[14px] font-semibold leading-5">{summary.label}</h2>
            <StatusPill tone={summary.tone}>{adminCopy.components.previewSnapshot}</StatusPill>
          </div>
          <p className="mt-4 text-[34px] font-semibold leading-none">{summary.value}</p>
          <p className="mt-2 text-[13px] leading-5 text-proph-ink-muted-80">{summary.detail}</p>
        </article>
      ))}
    </section>
  );
}
