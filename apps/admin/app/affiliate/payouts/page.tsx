import { AdminShell } from "@/components/admin-shell";
import { DomainScope } from "@/components/domain-scope";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { getOpsRoute } from "@/lib/ops-facade";

const payoutRequests = [
  {
    id: "PAY-2026-0514-001",
    affiliate: "AFF-001",
    amount: "USDT 1,284.00",
    requestedAt: "2026-05-14 09:20",
    status: "approved",
  },
  {
    id: "PAY-2026-0514-002",
    affiliate: "AFF-014",
    amount: "USDT 560.00",
    requestedAt: "2026-05-14 10:05",
    status: "reviewing",
  },
  {
    id: "PAY-2026-0513-008",
    affiliate: "AFF-022",
    amount: "USDT 120.00",
    requestedAt: "2026-05-13 18:42",
    status: "rejected",
  },
] as const;

export default function AffiliatePayoutsPage() {
  const route = getOpsRoute("affiliate-review");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {(viewer) => (
        <AdminShell
          eyebrow={route.label}
          title={adminCopy.affiliateAdmin.payoutsTitle}
          description={adminCopy.affiliateAdmin.payoutsDescription}
          role={viewer.role}
        >
          <div className="space-y-8">
            <DomainScope domain={route.domain} role={viewer.role} />
            <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div>
                  <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                    撥款申請佇列
                  </h2>
                  <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {adminCopy.affiliateAdmin.modulePlaceholder}
                  </p>
                </div>
                <StatusPill tone="neutral">申請 3</StatusPill>
              </div>
              <div className="mt-6 overflow-x-auto">
                <table className="min-w-full text-left text-[14px] leading-[1.43] tracking-[-0.224px]">
                  <thead className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
                    <tr>
                      <th className="border-b border-proph-hairline py-3 pr-4">
                        申請
                      </th>
                      <th className="border-b border-proph-hairline px-4 py-3">
                        代理
                      </th>
                      <th className="border-b border-proph-hairline px-4 py-3">
                        金額
                      </th>
                      <th className="border-b border-proph-hairline px-4 py-3">
                        申請時間
                      </th>
                      <th className="border-b border-proph-hairline py-3 pl-4">
                        審查結果
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {payoutRequests.map((item) => (
                      <tr key={item.id}>
                        <td className="border-b border-proph-divider-soft py-4 pr-4 font-mono">
                          {item.id}
                        </td>
                        <td className="border-b border-proph-divider-soft px-4 py-4 font-mono">
                          {item.affiliate}
                        </td>
                        <td className="border-b border-proph-divider-soft px-4 py-4">
                          {item.amount}
                        </td>
                        <td className="border-b border-proph-divider-soft px-4 py-4">
                          {item.requestedAt}
                        </td>
                        <td className="border-b border-proph-divider-soft py-4 pl-4">
                          {item.status}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          </div>
        </AdminShell>
      )}
    </RoleGate>
  );
}
