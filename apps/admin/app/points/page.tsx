import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { getOpsRoute } from "@/lib/ops-facade";

export default function PointsPage() {
  const route = getOpsRoute("points-exceptions");

  return (
    <AdminShell title={route.label} description={route.summary}>
      <DomainScope domain={route.domain} />
    </AdminShell>
  );
}
