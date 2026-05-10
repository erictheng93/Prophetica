import Link from "next/link";
import { adminCopy } from "@/lib/admin-copy";
import {
  formatBps,
  formatMinor,
  type AffiliateDetail,
  type AffiliatePayoutRow,
  type AffiliateRiskFlag,
} from "@/lib/affiliates";
import type { Role } from "@/lib/rbac";
import { AffiliateStatusPill } from "./affiliate-status-pill";
import { StatusPill } from "./status-pill";

const riskLabel: Record<AffiliateRiskFlag, string> = {
  "self-referral": adminCopy.affiliateAdmin.riskSelfReferral,
  "burst-signup": adminCopy.affiliateAdmin.riskBurstSignup,
  "geo-mismatch": adminCopy.affiliateAdmin.riskGeoMismatch,
  "kyc-tier1-only": adminCopy.affiliateAdmin.riskKycTier1Only,
};

const payoutTone: Record<AffiliatePayoutRow["status"], "neutral" | "warning" | "success" | "critical"> = {
  settled: "success",
  pending: "warning",
  blocked: "critical",
};

const payoutLabel: Record<AffiliatePayoutRow["status"], string> = {
  settled: "已撥款",
  pending: "待撥款",
  blocked: "受阻",
};

interface AffiliateDetailViewProps {
  affiliate: AffiliateDetail;
  viewerRole: Role;
}

export function AffiliateDetailView({ affiliate, viewerRole }: AffiliateDetailViewProps) {
  const isAdmin = viewerRole === "admin";

  return (
    <div className="space-y-8">
      <Link
        href="/affiliate"
        className="-mx-2 inline-flex min-h-11 items-center rounded-proph-sm px-2 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-primary transition-colors hover:bg-proph-canvas-parchment active:bg-proph-divider-soft"
      >
        ← {adminCopy.affiliateAdmin.detailBackToList}
      </Link>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
              {adminCopy.affiliateAdmin.detailEyebrow}
            </p>
            <h2 className="mt-2 text-[28px] font-semibold leading-[1.14] tracking-[-0.374px]">{affiliate.displayName}</h2>
            <p className="mt-1 font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">
              {affiliate.id} · {affiliate.emailMasked}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <AffiliateStatusPill status={affiliate.status} />
            {affiliate.riskFlags.map((flag) => (
              <StatusPill key={flag} tone="warning">
                {riskLabel[flag]}
              </StatusPill>
            ))}
          </div>
        </div>

        <dl className="mt-6 grid gap-4 md:grid-cols-4">
          <KeyValue label={adminCopy.affiliateAdmin.columnTier} value={`Tier ${affiliate.tier}`} />
          <KeyValue label={adminCopy.affiliateAdmin.columnRate} value={formatBps(affiliate.commissionRateBps)} />
          <KeyValue
            label={adminCopy.affiliateAdmin.columnSubTree}
            value={`${affiliate.subTreeSize} 人 / 活躍 ${affiliate.activeSubCount}`}
          />
          <KeyValue
            label={adminCopy.affiliateAdmin.columnPendingPayout}
            value={formatMinor(affiliate.pendingPayoutMinor, affiliate.pendingPayoutCurrency)}
          />
          <KeyValue
            label="上層 Sponsor"
            value={affiliate.ownerSponsorId ?? "—"}
            mono={Boolean(affiliate.ownerSponsorId)}
          />
          <KeyValue
            label={adminCopy.affiliateAdmin.columnLastActivity}
            value={new Date(affiliate.lastActivityAt).toLocaleString("zh-Hant")}
          />
        </dl>
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.detailSubTreeHeading}
        </h3>
        <ul className="mt-4 grid gap-3 md:grid-cols-3">
          {affiliate.subTreeBreakdown.map((tier) => (
            <li
              key={tier.tier}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
            >
              <p className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">Tier {tier.tier}</p>
              <p className="mt-3 text-[28px] font-semibold leading-[1.14] tracking-[-0.374px]">{tier.count}</p>
              <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                活躍 {tier.activeCount} · 本月貢獻 {formatMinor(tier.mtdContributionMinor, "USDT")}
              </p>
            </li>
          ))}
        </ul>
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.detailPayoutsHeading}
        </h3>
        {affiliate.recentPayouts.length === 0 ? (
          <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">尚無撥款紀錄。</p>
        ) : (
          <ul className="mt-4 divide-y divide-proph-hairline">
            {affiliate.recentPayouts.map((payout) => (
              <li
                key={payout.id}
                className="grid gap-2 py-3 md:grid-cols-[2fr_auto_auto_auto]"
              >
                <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80">{payout.id}</span>
                <span className="text-[17px] font-semibold tracking-[-0.374px]">
                  {formatMinor(payout.amountMinor, payout.currency)}
                </span>
                <StatusPill tone={payoutTone[payout.status]}>{payoutLabel[payout.status]}</StatusPill>
                <span className="text-[14px] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                  {payout.settledAt}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.detailRateHistoryHeading}
        </h3>
        {affiliate.rateHistory.length === 0 ? (
          <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {adminCopy.affiliateAdmin.detailRateHistoryEmpty}
          </p>
        ) : (
          <ul className="mt-4 divide-y divide-proph-hairline">
            {affiliate.rateHistory.map((change) => (
              <li
                key={`${change.effectiveAt}-${change.previousBps}-${change.nextBps}`}
                className="grid gap-2 py-3 md:grid-cols-[1fr_auto_auto]"
              >
                <span className="text-[14px] tracking-[-0.224px]">
                  <span className="text-proph-ink-muted-80">{change.effectiveAt}</span> · {change.reason}
                </span>
                <span className="font-mono text-[13px] tracking-[-0.224px]">
                  {formatBps(change.previousBps)} → {formatBps(change.nextBps)}
                </span>
                <span className="font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
                  by {change.changedBy}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.detailAuditHeading}
        </h3>
        {affiliate.auditCorrelationIds.length === 0 ? (
          <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {adminCopy.affiliateAdmin.detailAuditEmpty}
          </p>
        ) : (
          <ul className="mt-4 grid gap-2 md:grid-cols-2">
            {affiliate.auditCorrelationIds.map((id) => (
              <li
                key={id}
                className="rounded-proph-md border border-proph-divider-soft bg-proph-surface-pearl p-3 font-mono text-[13px] tracking-[-0.224px] text-proph-ink-muted-80"
              >
                {id}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {adminCopy.affiliateAdmin.detailActionsHeading}
        </h3>
        {!isAdmin && (
          <p className="mt-3 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            {adminCopy.affiliateAdmin.actionsManagerNote}
          </p>
        )}
        <div className="mt-4 grid gap-3 md:grid-cols-2 lg:grid-cols-4">
          <ActionButton
            label={
              affiliate.status === "frozen"
                ? adminCopy.affiliateAdmin.actionUnfreezeLabel
                : adminCopy.affiliateAdmin.actionFreezeLabel
            }
            disabled
            tone={affiliate.status === "frozen" ? "primary" : "warning"}
          />
          <ActionButton label={adminCopy.affiliateAdmin.actionAdjustRateLabel} disabled tone="primary" />
          <ActionButton label={adminCopy.affiliateAdmin.actionRequestAuditLabel} disabled tone="neutral" />
        </div>
        <p className="mt-4 text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {adminCopy.affiliateAdmin.actionDisabledTooltip}
        </p>
      </section>
    </div>
  );
}

function KeyValue({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-[11px] uppercase tracking-[0.08em] text-proph-ink-muted-48">{label}</dt>
      <dd
        className={
          mono
            ? "font-mono text-[14px] tracking-[-0.224px] text-proph-ink"
            : "text-[14px] tracking-[-0.224px] text-proph-ink"
        }
      >
        {value}
      </dd>
    </div>
  );
}

function ActionButton({
  label,
  disabled,
  tone,
}: {
  label: string;
  disabled?: boolean;
  tone: "primary" | "warning" | "neutral";
}) {
  const toneClass =
    tone === "primary"
      ? "bg-proph-primary text-proph-on-primary"
      : tone === "warning"
        ? "bg-proph-canvas-parchment text-proph-ink border border-proph-feedback-warning"
        : "bg-proph-surface-pearl text-proph-ink border border-proph-hairline";
  return (
    <button
      type="button"
      disabled={disabled}
      title={disabled ? adminCopy.affiliateAdmin.actionDisabledTooltip : undefined}
      className={`inline-flex h-11 items-center justify-center rounded-proph-pill px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] transition-opacity disabled:cursor-not-allowed disabled:opacity-50 ${toneClass}`}
    >
      {label}
    </button>
  );
}
