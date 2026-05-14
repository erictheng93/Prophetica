import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { MemberManagement } from "@/components/member-management";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default function MembersPage() {
  const route = getOpsRoute("member-management");

  return (
    <RoleGate allow={["admin", "manager", "affiliate"]}>
      {(viewer) => (
        <AdminShell title={route.label} description={route.summary} role={viewer.role}>
          <div className="space-y-8">
            <DomainScope domain={route.domain} role={viewer.role} />
            <MemberManagement
              viewerAffiliateId={viewer.affiliateId}
              viewerRole={viewer.role}
            />
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
