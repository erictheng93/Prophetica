import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { getOpsRoute } from "@/lib/ops-facade";

export default function ApprovalsPage() {
  const route = getOpsRoute("manual-approvals");

  return (
    <AdminShell title={route.label} description={route.summary}>
      <DomainScope domain={route.domain} />
    </AdminShell>
  );
}
