import { PointsOperations } from "@/components/points-operations";
import { RoleGate } from "@/components/role-gate";

export default function AffiliatePointsPage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {(viewer) => <PointsOperations viewerRole={viewer.role} />}
    </RoleGate>
  );
}
