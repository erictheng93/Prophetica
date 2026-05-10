import { ProphLinkButton } from "@prophetica/ui";
import { adminCopy } from "@/lib/admin-copy";
import type { OpsRoute } from "@/lib/ops-facade";
import { capabilityFor, type Role } from "@/lib/rbac";
import { StatusPill } from "./status-pill";

interface RouteEntryCardProps {
  route: OpsRoute;
  role: Role;
}

const capabilityLabel: Record<NonNullable<ReturnType<typeof capabilityFor>>, string> = {
  approve: "可核准",
  propose: "可建立提案",
  view: "唯讀",
};

export function RouteEntryCard({ route, role }: RouteEntryCardProps) {
  const capability = capabilityFor(role, route.domain);
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <StatusPill tone={route.tone}>
          {route.openItems} {adminCopy.components.openItems}
        </StatusPill>
        <span className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
          {adminCopy.components.entryLabel}
        </span>
      </div>
      <h2 className="mt-4 text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">{route.label}</h2>
      <p className="mt-2 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">{route.summary}</p>
      <p className="mt-4 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{route.scope}</p>
      <div className="mt-6 flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-col gap-1">
          <span className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">{route.owner}</span>
          {capability && (
            <span className="text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              {capabilityLabel[capability]}
            </span>
          )}
        </div>
        <ProphLinkButton href={route.href} variant="dark-utility">
          {adminCopy.components.open}
        </ProphLinkButton>
      </div>
    </article>
  );
}
