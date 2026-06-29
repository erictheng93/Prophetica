import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { fetchAffiliateDashboard } from "@/lib/affiliate-management-bff";
import { formatMinor } from "@/lib/affiliates";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AffiliateSubTreePage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {async (viewer) => {
        const dashboard = await fetchAffiliateDashboard(
          viewer,
          await getUpstreamSessionToken(),
        );
        return (
          <section
            aria-labelledby="sub-tree-title"
            className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
          >
            <h2
              id="sub-tree-title"
              className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
            >
              {adminCopy.affiliatePortal.subTreeHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.affiliatePortal.subTreeDescription}
            </p>
            {dashboard ? (
              <ul className="mt-5 divide-y divide-proph-hairline">
                {dashboard.referrals.map((referral) => (
                  <li
                    key={referral.referred_id}
                    className="grid gap-2 py-3 md:grid-cols-[1fr_auto_auto]"
                  >
                    <span className="font-mono text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                      {referral.masked_username}
                    </span>
                    <span className="text-[17px] font-semibold tracking-[-0.374px]">
                      {formatMinor(
                        referral.cumulative_commission_minor_units,
                        "USDT",
                      )}
                    </span>
                    <span className="text-[14px] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                      Day {referral.referred_at_day} · {referral.status}
                    </span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="mt-6 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                referral-service 尚未連線，無法讀取推薦鏈。
              </p>
            )}
          </section>
        );
      }}
    </RoleGate>
  );
}
