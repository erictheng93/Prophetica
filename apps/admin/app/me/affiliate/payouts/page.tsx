import { AffiliateShell } from "@/components/affiliate-shell";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";

const placeholderPayouts = [
  { id: "PAYOUT-2026-04", amount: "USDT 982.40", settledAt: "2026-04-15", status: "已撥款" },
  { id: "PAYOUT-2026-03", amount: "USDT 855.10", settledAt: "2026-03-15", status: "已撥款" },
  { id: "PAYOUT-2026-05", amount: "USDT 1,284.00", settledAt: "2026-05-15", status: "待撥款" },
];

export default function AffiliatePayoutsPage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {(viewer) => (
        <AffiliateShell affiliateId={viewer.affiliateId ?? "—"} active="payouts">
          <section
            aria-labelledby="payouts-title"
            className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
          >
            <h2 id="payouts-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.affiliatePortal.payoutsHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.affiliatePortal.payoutsDescription}
            </p>
            <ul className="mt-5 divide-y divide-proph-hairline">
              {placeholderPayouts.map((payout) => (
                <li key={payout.id} className="grid gap-2 py-3 md:grid-cols-[1fr_auto_auto]">
                  <span className="font-mono text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">{payout.id}</span>
                  <span className="text-[17px] font-semibold tracking-[-0.374px]">{payout.amount}</span>
                  <span className="text-[14px] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                    {payout.settledAt} · {payout.status}
                  </span>
                </li>
              ))}
            </ul>
            <p className="mt-6 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.affiliatePortal.placeholder}
            </p>
          </section>
        </AffiliateShell>
      )}
    </RoleGate>
  );
}
