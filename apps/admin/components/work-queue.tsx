import type { OpsQueueItem, OpsRoute, OpsTone } from "@/lib/ops-facade";
import { adminCopy } from "@/lib/admin-copy";
import { StatusPill } from "./status-pill";

const statusTone: Record<OpsQueueItem["status"], OpsTone> = {
  new: "critical",
  reviewing: "neutral",
  blocked: "warning",
  ready: "success",
};

interface WorkQueueProps {
  items: OpsQueueItem[];
  routes: OpsRoute[];
}

export function WorkQueue({ items, routes }: WorkQueueProps) {
  const labels = new Map(routes.map((route) => [route.domain, route.label]));

  return (
    <section aria-labelledby="work-queue-title">
      <div className="mb-4">
        <h2 id="work-queue-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.components.workQueueHeading}
        </h2>
        <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          {adminCopy.components.workQueueDescription}
        </p>
      </div>
      <div className="overflow-hidden rounded-proph-lg border border-proph-hairline">
        {items.map((item) => (
          <article
            className="grid gap-3 border-b border-proph-hairline bg-proph-canvas p-5 last:border-b-0 md:grid-cols-[1fr_auto]"
            key={item.id}
          >
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80">{item.id}</span>
                <StatusPill tone={statusTone[item.status]}>{adminCopy.components.status[item.status]}</StatusPill>
                <span className="text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-80">{labels.get(item.domain)}</span>
              </div>
              <h3 className="mt-2 text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">{item.title}</h3>
              <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{item.detail}</p>
            </div>
            <div className="text-left text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
              <p>{item.assignee}</p>
              <p>{item.age}</p>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
