import { notFound } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { AffiliateDetailView } from "@/components/affiliate-detail";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { getAffiliateDetail } from "@/lib/affiliates";

interface AffiliateDetailPageProps {
  params: Promise<{ id: string }>;
}

export default async function AffiliateDetailPage({ params }: AffiliateDetailPageProps) {
  const { id } = await params;
  const affiliate = getAffiliateDetail(id);
  if (!affiliate) {
    notFound();
  }

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          eyebrow={adminCopy.affiliateAdmin.detailEyebrow}
          title={affiliate.displayName}
          description={`${affiliate.id} · ${affiliate.emailMasked}`}
          role={viewer.role}
        >
          <AffiliateDetailView affiliate={affiliate} viewerRole={viewer.role} />
        </AdminShell>
      )}
    </RoleGate>
  );
}
