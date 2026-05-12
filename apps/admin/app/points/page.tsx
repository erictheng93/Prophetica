import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { PointsOperations } from "@/components/points-operations";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default function PointsPage() {
  const route = getOpsRoute("points-exceptions");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          title={route.label}
          description={route.summary}
          role={viewer.role}
        >
          <div className="space-y-8">
            <DomainScope domain={route.domain} role={viewer.role} />
            <PointsOperations viewerRole={viewer.role} />
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
