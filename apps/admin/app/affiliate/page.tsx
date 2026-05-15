import { AdminShell } from "@/components/admin-shell";
import { AffiliateCreateForm } from "@/components/affiliate-admin-forms";
import { AffiliateList } from "@/components/affiliate-list";
import { DomainScope } from "@/components/domain-scope";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { fetchAdminAffiliateList } from "@/lib/affiliate-management-bff";
import { getOpsRoute } from "@/lib/ops-facade";

export default function AffiliatePage() {
  const route = getOpsRoute("affiliate-review");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const affiliates = await fetchAdminAffiliateList(viewer);
        return (
          <AdminShell
            eyebrow={route.label}
            title={adminCopy.affiliateAdmin.listHeading}
            description={adminCopy.affiliateAdmin.listDescription}
            role={viewer.role}
          >
            <div className="space-y-8">
              <DomainScope domain={route.domain} role={viewer.role} />
              <AffiliateCreateForm role={viewer.role} />
              <AffiliateList affiliates={affiliates} />
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}
