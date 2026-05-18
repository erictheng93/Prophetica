import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default function KycRiskPage() {
  const route = getOpsRoute("kyc-risk-events");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          title={route.label}
          description={route.summary}
        >
          <></>
        </AdminShell>
      )}
    </RoleGate>
  );
}
