import { AffiliateShell } from "@/components/affiliate-shell";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";

const placeholderTiers = [
  { tier: "Tier 1（直推）", count: 12, active: 8, contribution: "USDT 612.00" },
  { tier: "Tier 2", count: 28, active: 15, contribution: "USDT 415.20" },
  { tier: "Tier 3", count: 41, active: 19, contribution: "USDT 256.80" },
];

export default function AffiliateSubTreePage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {(viewer) => (
        <AffiliateShell affiliateId={viewer.affiliateId ?? "—"} active="sub-tree">
          <section
            aria-labelledby="sub-tree-title"
            className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
          >
            <h2 id="sub-tree-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.affiliatePortal.subTreeHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.affiliatePortal.subTreeDescription}
            </p>
            <ul className="mt-5 grid gap-3 md:grid-cols-3">
              {placeholderTiers.map((tier) => (
                <li
                  key={tier.tier}
                  className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
                >
                  <h3 className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">{tier.tier}</h3>
                  <p className="mt-3 text-[28px] font-semibold leading-[1.14] tracking-[-0.374px]">{tier.count}</p>
                  <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    活躍 {tier.active} · 本月貢獻 {tier.contribution}
                  </p>
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
