import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default function ApprovalsPage() {
  const route = getOpsRoute("manual-approvals");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell title={route.label} description={route.summary} role={viewer.role}>
          <DomainScope domain={route.domain} role={viewer.role} />
        </AdminShell>
      )}
    </RoleGate>
  );
}
