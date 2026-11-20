import { AdminShell } from "@/components/admin-shell";
import { MemberManagement } from "@/components/member-management";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default async function MembersPage({
  searchParams,
}: {
  searchParams?: Promise<{ query?: string }>;
}) {
  const route = getOpsRoute("member-management");
  const params = await searchParams;
  const initialQuery = params?.query;

  return (
    <RoleGate allow={["admin", "manager", "affiliate"]}>
      {(viewer) => (
        <AdminShell title={route.label} description={route.summary}>
          <div className="space-y-8">
            <MemberManagement
              initialQuery={initialQuery}
              viewerAffiliateId={viewer.affiliateId}
              viewerRole={viewer.role}
            />
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
