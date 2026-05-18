import { AdminShell } from "@/components/admin-shell";
import { MemberManagement } from "@/components/member-management";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { getOpsRoute } from "@/lib/ops-facade";

export default function ApprovalsPage() {
  const route = getOpsRoute("manual-approvals");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          title={route.label}
          description={route.summary}
        >
          {route.inDevelopment ? (
            <div
              role="status"
              className="mb-6 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-4"
            >
              <p className="text-[14px] font-semibold leading-[1.43] tracking-[-0.224px] text-proph-ink">
                {adminCopy.approvals.inDevelopmentTitle}
              </p>
              <p className="mt-1 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {adminCopy.approvals.inDevelopmentBody}
              </p>
            </div>
          ) : null}
          <MemberManagement
            showActionQueue
            showMemberWorkspace={false}
            viewerRole={viewer.role}
          />
        </AdminShell>
      )}
    </RoleGate>
  );
}
