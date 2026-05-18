import { AdminShell } from "@/components/admin-shell";
import { MemberManagement } from "@/components/member-management";
import { RoleGate } from "@/components/role-gate";
import { getOpsRoute } from "@/lib/ops-facade";

export default async function MembersKycReviewPage({
  searchParams,
}: {
  searchParams?: Promise<{ query?: string }>;
}) {
  const route = getOpsRoute("member-management");
  const params = await searchParams;

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          title="KYC 審核"
          description="聚焦顯示待處理 KYC 審核案件與限制原因，便於快速覆核與追蹤。"
        >
          <div className="space-y-8">
            <MemberManagement
              initialQuery={params?.query}
              initialPendingStatus="all"
              showPendingCases
              viewerAffiliateId={viewer.affiliateId}
              viewerRole={viewer.role}
            />
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
