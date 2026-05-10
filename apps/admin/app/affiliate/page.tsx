import { AdminShell } from "@/components/admin-shell";
import { AffiliateList } from "@/components/affiliate-list";
import { DomainScope } from "@/components/domain-scope";
import { RoleGate } from "@/components/role-gate";
import { getAffiliatesSnapshot } from "@/lib/affiliates";
import { getOpsRoute } from "@/lib/ops-facade";

export default function AffiliatePage() {
  const route = getOpsRoute("affiliate-review");
  const affiliates = getAffiliatesSnapshot();

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell title={route.label} description={route.summary} role={viewer.role}>
          <div className="space-y-8">
            <DomainScope domain={route.domain} role={viewer.role} />
            <AffiliateList affiliates={affiliates} />
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
