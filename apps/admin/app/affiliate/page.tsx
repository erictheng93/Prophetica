import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { getOpsRoute } from "@/lib/ops-facade";

export default function AffiliatePage() {
  const route = getOpsRoute("affiliate-review");

  return (
    <AdminShell title={route.label} description={route.summary}>
      <DomainScope domain={route.domain} />
    </AdminShell>
  );
}
