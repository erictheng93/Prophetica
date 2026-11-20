import type { OpsQueueSummary } from "@/lib/ops-facade";
import { adminCopy } from "@/lib/admin-copy";
import { StatusPill } from "./status-pill";

interface QueueSummaryProps {
  summaries: OpsQueueSummary[];
}

export function QueueSummary({ summaries }: QueueSummaryProps) {
  return (
    <section
      aria-label="Queue summary"
      className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4"
    >
      {summaries.map((summary) => (
        <article
          className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1"
          key={summary.label}
        >
          <div className="flex items-center justify-between gap-3">
            <h2 className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">
              {summary.label}
            </h2>
            <StatusPill tone={summary.tone}>
              {adminCopy.components.previewSnapshot}
            </StatusPill>
          </div>
          <p className="mt-4 font-display text-[28px] font-semibold leading-[1.07] tracking-[-0.374px] sm:text-[34px]">
            {summary.value}
          </p>
          <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {summary.detail}
          </p>
        </article>
      ))}
    </section>
  );
}
