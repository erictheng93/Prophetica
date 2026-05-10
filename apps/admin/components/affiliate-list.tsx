import Link from "next/link";
import { adminCopy } from "@/lib/admin-copy";
import {
  formatBps,
  formatMinor,
  type AffiliateRiskFlag,
  type AffiliateSummary,
} from "@/lib/affiliates";
import { AffiliateStatusPill } from "./affiliate-status-pill";

interface AffiliateListProps {
  affiliates: readonly AffiliateSummary[];
}

const riskLabel: Record<AffiliateRiskFlag, string> = {
  "self-referral": adminCopy.affiliateAdmin.riskSelfReferral,
  "burst-signup": adminCopy.affiliateAdmin.riskBurstSignup,
  "geo-mismatch": adminCopy.affiliateAdmin.riskGeoMismatch,
  "kyc-tier1-only": adminCopy.affiliateAdmin.riskKycTier1Only,
};

export function AffiliateList({ affiliates }: AffiliateListProps) {
  return (
    <section
      aria-labelledby="affiliate-list-title"
      className="rounded-proph-lg border border-proph-hairline bg-proph-canvas"
    >
      <header className="px-5 pt-5 md:px-6 md:pt-6">
        <h2 id="affiliate-list-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.listHeading}
        </h2>
        <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          {adminCopy.affiliateAdmin.listDescription}
        </p>
      </header>
      <ul className="mt-4 divide-y divide-proph-hairline">
        {affiliates.map((row) => (
          <li key={row.id}>
            <Link
              href={`/affiliate/${row.id}`}
              className="grid min-h-[88px] gap-3 px-5 py-5 transition-colors hover:bg-proph-surface-pearl active:bg-proph-canvas-parchment md:px-6 md:grid-cols-[160px_1fr_auto_auto]"
            >
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">{row.id}</span>
                <span className="text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">{row.displayName}</span>
                <span className="text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                  {row.emailMasked}
                </span>
              </div>

              <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                <Cell label={adminCopy.affiliateAdmin.columnTier} value={`Tier ${row.tier}`} />
                <Cell
                  label={adminCopy.affiliateAdmin.columnRate}
                  value={formatBps(row.commissionRateBps)}
                />
                <Cell
                  label={adminCopy.affiliateAdmin.columnSubTree}
                  value={`${row.subTreeSize} 人 / 活躍 ${row.activeSubCount}`}
                />
                <Cell
                  label={adminCopy.affiliateAdmin.columnLastActivity}
                  value={new Date(row.lastActivityAt).toLocaleString("zh-Hant", {
                    year: "numeric",
                    month: "2-digit",
                    day: "2-digit",
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                />
              </div>

              <div className="flex flex-col items-start gap-2 md:items-end">
                <AffiliateStatusPill status={row.status} />
                <span className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">
                  {formatMinor(row.pendingPayoutMinor, row.pendingPayoutCurrency)}
                </span>
                <span className="text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                  {adminCopy.affiliateAdmin.columnPendingPayout}
                </span>
              </div>

              <div className="flex flex-wrap items-start gap-1 md:items-end md:justify-end">
                {row.riskFlags.length === 0 ? (
                  <span className="text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                    {adminCopy.affiliateAdmin.riskNone}
                  </span>
                ) : (
                  row.riskFlags.map((flag) => (
                    <span
                      key={flag}
                      className="inline-flex items-center rounded-proph-pill border border-proph-feedback-warning bg-proph-canvas-parchment px-2 py-1 text-[11px] font-semibold leading-none tracking-[-0.12px] text-proph-ink"
                    >
                      {riskLabel[flag]}
                    </span>
                  ))
                )}
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Cell({ label, value }: { label: string; value: string }) {
  return (
    <span className="inline-flex flex-col">
      <span className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">{label}</span>
      <span className="text-proph-ink">{value}</span>
    </span>
  );
}
