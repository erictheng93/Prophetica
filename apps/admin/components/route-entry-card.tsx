import { ProphLinkButton } from "@prophetica/ui";
import { adminCopy } from "@/lib/admin-copy";
import type { OpsRoute } from "@/lib/ops-facade";
import { StatusPill } from "./status-pill";

interface RouteEntryCardProps {
  route: OpsRoute;
}

export function RouteEntryCard({ route }: RouteEntryCardProps) {
  return (
    <article className="rounded-proph-sm border border-proph-hairline bg-proph-canvas p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <StatusPill tone={route.tone}>
          {route.openItems} {adminCopy.components.openItems}
        </StatusPill>
        <span className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">工作區</span>
      </div>
      <h2 className="mt-4 text-[21px] font-semibold leading-7">{route.label}</h2>
      <p className="mt-2 text-[14px] leading-5 text-proph-ink-muted-80">{route.summary}</p>
      <p className="mt-4 text-[13px] leading-5 text-proph-ink-muted-80">{route.scope}</p>
      <div className="mt-5 flex flex-wrap items-center justify-between gap-3">
        <span className="text-[13px] font-semibold text-proph-ink">{route.owner}</span>
        <ProphLinkButton href={route.href} variant="dark-utility">
          {adminCopy.components.open}
        </ProphLinkButton>
      </div>
    </article>
  );
}
