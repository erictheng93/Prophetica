import { getAdminOperationsSnapshot, type OpsDomainId } from "@/lib/ops-facade";
import { adminCopy } from "@/lib/admin-copy";
import { capabilityFor, type Role } from "@/lib/rbac";
import { QueueSummary } from "./queue-summary";
import { StatusPill } from "./status-pill";
import { WorkQueue } from "./work-queue";

interface DomainScopeProps {
  domain: OpsDomainId;
  role: Role;
}

const capabilityCopy: Record<NonNullable<ReturnType<typeof capabilityFor>>, string> = {
  approve: "你的角色可在此頁面進行終審核准。所有核准動作均寫入 audit trail。",
  propose: "你的角色可建立提案，待 admin 終審後才會生效。",
  view: "你的角色為唯讀。如需動作，請通知 admin 或提交工單。",
};

export function DomainScope({ domain, role }: DomainScopeProps) {
  const snapshot = getAdminOperationsSnapshot();
  const route = snapshot.routes.find((item) => item.domain === domain);
  const items = snapshot.queue.filter((item) => item.domain === domain);
  const capability = capabilityFor(role, domain);
  const scopeNote =
    domain === "member-management"
      ? "此工作區可新增與編輯會員；admin / manager 可刪除，affiliate 僅能管理自己代理鏈歸屬範圍內的會員。"
      : adminCopy.components.readOnlyScope;

  if (!route) {
    return null;
  }

  return (
    <div className="space-y-8">
      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <div className="flex flex-wrap items-center gap-3">
          <StatusPill tone={route.tone}>
            {route.openItems} {adminCopy.components.openItems}
          </StatusPill>
          <span className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80">{route.owner}</span>
        </div>
        <h2 className="mt-4 text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">{adminCopy.components.operationalScope}</h2>
        <p className="mt-2 max-w-3xl text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">{route.scope}</p>
        <p className="mt-4 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          {scopeNote}
        </p>
        {capability && (
          <p className="mt-3 max-w-3xl rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            {capabilityCopy[capability]}
          </p>
        )}
      </section>

      <QueueSummary summaries={snapshot.summaries} />
      <WorkQueue items={items.length > 0 ? items : snapshot.queue.slice(0, 2)} routes={snapshot.routes} />
    </div>
  );
}
