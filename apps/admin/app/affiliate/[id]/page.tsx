import { notFound } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { AffiliateDetailView } from "@/components/affiliate-detail";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { fetchAdminAffiliateDetail } from "@/lib/affiliate-management-bff";

interface AffiliateDetailPageProps {
  params: Promise<{ id: string }>;
}

export default async function AffiliateDetailPage({
  params,
}: AffiliateDetailPageProps) {
  const { id } = await params;

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const affiliate = await fetchAdminAffiliateDetail(viewer, id);
        if (!affiliate) {
          notFound();
        }
        return (
          <AdminShell
            eyebrow={adminCopy.affiliateAdmin.detailEyebrow}
            title={affiliate.displayName}
            description={`${affiliate.id} · ${affiliate.emailMasked}`}
          >
            <AffiliateDetailView
              affiliate={affiliate}
              viewerRole={viewer.role}
            />
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}
