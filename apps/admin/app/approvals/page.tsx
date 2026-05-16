import { AdminShell } from "@/components/admin-shell";
import { MemberManagement } from "@/components/member-management";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default function ApprovalsPage() {
  const route = getOpsRoute("manual-approvals");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          title={route.label}
          description={route.summary}
          role={viewer.role}
        >
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
