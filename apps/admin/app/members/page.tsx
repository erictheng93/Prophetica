import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { getOpsRoute } from "@/lib/ops-facade";

export default function MembersPage() {
  const route = getOpsRoute("member-management");

  return (
    <AdminShell title={route.label} description={route.summary}>
      <DomainScope domain={route.domain} />
    </AdminShell>
  );
}
