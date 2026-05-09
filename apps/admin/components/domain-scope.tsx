import { getAdminOperationsSnapshot, type OpsDomainId } from "@/lib/ops-facade";
import { QueueSummary } from "./queue-summary";
import { StatusPill } from "./status-pill";
import { WorkQueue } from "./work-queue";

interface DomainScopeProps {
  domain: OpsDomainId;
}

export function DomainScope({ domain }: DomainScopeProps) {
  const snapshot = getAdminOperationsSnapshot();
  const route = snapshot.routes.find((item) => item.domain === domain);
  const items = snapshot.queue.filter((item) => item.domain === domain);

  if (!route) {
    return null;
  }

  return (
    <div className="space-y-8">
      <section className="rounded-proph-sm border border-proph-hairline bg-proph-canvas p-5">
        <div className="flex flex-wrap items-center gap-3">
          <StatusPill tone={route.tone}>{route.openItems} open</StatusPill>
          <span className="text-[13px] font-semibold text-proph-ink-muted-80">{route.owner}</span>
        </div>
        <h2 className="mt-4 text-[24px] font-semibold leading-8">Operational scope</h2>
        <p className="mt-2 max-w-3xl text-[15px] leading-6 text-proph-ink-muted-80">{route.scope}</p>
        <p className="mt-4 max-w-3xl text-[14px] leading-5 text-proph-ink-muted-80">
          This entry surface is read-only. Operators can review context, prepare evidence, and route decisions to
          approved systems of record.
        </p>
      </section>

      <QueueSummary summaries={snapshot.summaries} />
      <WorkQueue items={items.length > 0 ? items : snapshot.queue.slice(0, 2)} routes={snapshot.routes} />
    </div>
  );
}
