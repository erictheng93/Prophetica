import { RoleGate } from "@/components/role-gate";
import { fetchAffiliateDashboard } from "@/lib/affiliate-management-bff";
import { formatMinor } from "@/lib/affiliates";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AffiliateHomePage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {async (viewer) => {
        const dashboard = await fetchAffiliateDashboard(
          viewer,
          await getUpstreamSessionToken(),
        );
        return (
          <div className="space-y-8">
            <section
              aria-labelledby="affiliate-summary-title"
              className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8"
            >
              <h2
                id="affiliate-summary-title"
                className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
              >
                本月概覽
              </h2>
              {dashboard ? (
                <div className="mt-4 grid gap-4 md:grid-cols-3">
                  <Stat
                    label="成功推薦"
                    value={String(dashboard.summary.successful_referrals_count)}
                    detail={`推薦碼 ${dashboard.referral_code}`}
                  />
                  <Stat
                    label="總佣金"
                    value={formatMinor(
                      dashboard.summary.total_rewards_minor_units,
                      "USDT",
                    )}
                    detail="註冊獎勵與交易佣金"
                  />
                  <Stat
                    label="近 30 日佣金"
                    value={formatMinor(
                      dashboard.summary.last_30d_commission_minor_units,
                      "USDT",
                    )}
                    detail="由 referral-service 即時計算"
                  />
                </div>
              ) : (
                <p className="mt-6 max-w-3xl rounded-proph-md border border-proph-divider-soft bg-proph-surface-pearl p-4 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 shadow-sm">
                  referral-service 尚未連線，無法讀取代理概覽。
                </p>
              )}
            </section>
          </div>
        );
      }}
    </RoleGate>
  );
}

function Stat({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1">
      <h3 className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">
        {label}
      </h3>
      <p className="mt-3 font-display text-[28px] font-semibold leading-[1.14] tracking-[-0.374px] sm:text-[34px]">
        {value}
      </p>
      <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {detail}
      </p>
    </article>
  );
}
