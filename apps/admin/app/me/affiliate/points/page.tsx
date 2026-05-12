import { AffiliateShell } from "@/components/affiliate-shell";
import { PointsOperations } from "@/components/points-operations";
import { RoleGate } from "@/components/role-gate";

export default function AffiliatePointsPage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {(viewer) => (
        <AffiliateShell affiliateId={viewer.affiliateId ?? "—"} active="points">
          <PointsOperations viewerRole={viewer.role} />
        </AffiliateShell>
      )}
    </RoleGate>
  );
}
