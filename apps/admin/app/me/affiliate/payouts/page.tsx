import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import { fetchAffiliateDashboard } from "@/lib/affiliate-management-bff";
import { formatMinor } from "@/lib/affiliates";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AffiliatePayoutsPage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {async (viewer) => {
        const dashboard = await fetchAffiliateDashboard(
          viewer,
          await getUpstreamSessionToken(),
        );
        return (
          <section
            aria-labelledby="payouts-title"
            className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
          >
            <h2
              id="payouts-title"
              className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
            >
              {adminCopy.affiliatePortal.payoutsHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.affiliatePortal.payoutsDescription}
            </p>
            {dashboard ? (
              <div className="mt-5 grid gap-4 md:grid-cols-3">
                <Stat
                  label="累計佣金"
                  value={formatMinor(
                    dashboard.summary.total_rewards_minor_units,
                    "USDT",
                  )}
                />
                <Stat
                  label="註冊獎勵"
                  value={formatMinor(
                    dashboard.summary.signup_bonus_minor_units,
                    "USDT",
                  )}
                />
                <Stat
                  label="交易佣金"
                  value={formatMinor(
                    dashboard.summary.trade_commission_minor_units,
                    "USDT",
                  )}
                />
              </div>
            ) : (
              <p className="mt-6 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                referral-service 尚未連線，無法讀取佣金資料。
              </p>
            )}
          </section>
        );
      }}
    </RoleGate>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5">
      <h3 className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">
        {label}
      </h3>
      <p className="mt-3 text-[24px] font-semibold leading-[1.14] tracking-[-0.374px]">
        {value}
      </p>
    </article>
  );
}
