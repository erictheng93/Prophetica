import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import { StatusPill } from "@/components/status-pill";
import { adminCopy } from "@/lib/admin-copy";
import { fetchAffiliatePayoutReviews } from "@/lib/affiliate-management-bff";
import { formatMinor } from "@/lib/affiliates";
import { getOpsRoute } from "@/lib/ops-facade";

export default function AffiliatePayoutsPage() {
  const route = getOpsRoute("affiliate-review");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const payoutRequests = await fetchAffiliatePayoutReviews(viewer);
        return (
          <AdminShell
            eyebrow={route.label}
            title={adminCopy.affiliateAdmin.payoutsTitle}
            description={adminCopy.affiliateAdmin.payoutsDescription}
            role={viewer.role}
          >
            <div className="space-y-8">
              <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div>
                    <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                      撥款申請佇列
                    </h2>
                    <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                      來自 referral-service 的待處理代理撥款申請。
                    </p>
                  </div>
                  <StatusPill tone="neutral">
                    申請 {payoutRequests.length}
                  </StatusPill>
                </div>
                {payoutRequests.length ? (
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
                              {item.affiliateId}
                            </td>
                            <td className="border-b border-proph-divider-soft px-4 py-4">
                              {formatMinor(item.amountMinor, item.currency)}
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
                ) : (
                  <p className="mt-6 rounded-proph-md border border-proph-hairline bg-proph-surface-pearl p-4 text-[14px] leading-[1.43] text-proph-ink-muted-80">
                    目前沒有待處理撥款申請，或 referral-service 尚未連線。
                  </p>
                )}
              </section>
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}
