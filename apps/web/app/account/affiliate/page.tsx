import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";
import { formatPoints } from "@/lib/mock-markets";
import {
  fetchAffiliateDashboard,
  type AffiliateDashboard,
  type AffiliateReferralListItem,
  type AffiliateReferralStatus,
} from "@/lib/affiliate-api";
import { CopyButton } from "./copy-button";

// SPEC-011 v0.2.1 §5.1 — 玩家自助介面（推薦清單，flat list，**不是樹**）
// + §9.2 frontend acceptance.
// PII visibility constrained to masked usernames (server-side
// 「前 3 字 + 後 1 字」 rule); see referral-service masking.
export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("accountAffiliate.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

type Copy = Awaited<ReturnType<typeof getTranslations<"accountAffiliate">>>;

export default async function AccountAffiliatePage() {
  const [t, sectionFooterColumns, dashboard] = await Promise.all([
    getTranslations("accountAffiliate"),
    getFooterColumns(["player", "legal", "support"]),
    fetchAffiliateDashboard(),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />

      <SourceBanner t={t} dashboard={dashboard} />

      <SummaryCards t={t} dashboard={dashboard} />

      <ShareSection t={t} dashboard={dashboard} />

      <ReferralListSection t={t} referrals={dashboard.referrals} />

      <NotMlmReminder t={t} />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function SourceBanner({ t, dashboard }: { t: Copy; dashboard: AffiliateDashboard }) {
  if (dashboard.source === "api-gateway") {
    return null;
  }

  const messageKey =
    dashboard.source === "fallback:affiliate-not-found"
      ? "demoBanner.notFoundBody"
      : "demoBanner.unavailableBody";

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
        <p className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          <strong className="font-semibold text-proph-ink">
            {t("demoBanner.label")}
          </strong>{" "}
          {t(messageKey)}
        </p>
      </div>
    </section>
  );
}

function SummaryCards({ t, dashboard }: { t: Copy; dashboard: AffiliateDashboard }) {
  const cards = [
    {
      label: t("summary.count.label"),
      value: dashboard.summary.successfulReferralsCount.toLocaleString("zh-Hant"),
    },
    {
      label: t("summary.total.label"),
      value: formatPoints(dashboard.summary.totalRewardsPoints),
    },
    {
      label: t("summary.last30d.label"),
      value: formatPoints(dashboard.summary.last30dCommissionPoints),
    },
    {
      label: t("summary.last12m.label"),
      value: formatPoints(dashboard.summary.last12mCommissionPoints),
    },
  ];

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
            {t("summary.heading")}
          </h2>
          <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("summary.subtitle")}
          </p>
        </header>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          {cards.map((card) => (
            <article
              key={card.label}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="mt-2 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-ink sm:text-[34px]">
                {card.value}
              </div>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                {card.label}
              </div>
            </article>
          ))}
        </div>

        <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          {t("summary.footnote")}
        </p>
      </div>
    </section>
  );
}

function ShareSection({ t, dashboard }: { t: Copy; dashboard: AffiliateDashboard }) {
  const hasCode = dashboard.referralCode.length > 0;

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-6">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
            {t("share.heading")}
          </h2>
          <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("share.intro")}
          </p>
        </header>

        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 md:p-8">
          <div className="grid grid-cols-1 gap-6 md:grid-cols-[1fr_auto] md:items-start">
            <div className="min-w-0">
              <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {t("share.codeLabel")}
              </div>
              <div className="mt-2 break-all font-mono text-[21px] tabular-nums tracking-[-0.231px] text-proph-ink">
                {hasCode ? dashboard.referralCode : t("share.codePlaceholder")}
              </div>
            </div>
            <CopyButton
              variant="outline"
              value={dashboard.referralCode}
              label={t("share.copyCode")}
              copiedLabel={t("share.copied")}
              ariaLabel={t("share.copyCodeAria")}
            />
          </div>

          <div className="mt-6 grid grid-cols-1 gap-6 border-t border-proph-divider-soft pt-6 md:grid-cols-[1fr_auto] md:items-start">
            <div className="min-w-0">
              <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {t("share.linkLabel")}
              </div>
              <div className="mt-2 break-all font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                {hasCode ? dashboard.shareUrl : t("share.linkPlaceholder")}
              </div>
            </div>
            <CopyButton
              variant="primary"
              value={dashboard.shareUrl}
              label={t("share.copyLink")}
              copiedLabel={t("share.copied")}
              ariaLabel={t("share.copyLinkAria")}
            />
          </div>

          <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("share.disclaimer")}
          </p>
        </article>
      </div>
    </section>
  );
}

function ReferralListSection({
  t,
  referrals,
}: {
  t: Copy;
  referrals: AffiliateReferralListItem[];
}) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-6 flex items-baseline justify-between gap-4">
          <div>
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
              {t("referrals.heading")}
            </h2>
            <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("referrals.subtitle")}
            </p>
          </div>
          <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("referrals.count", { count: referrals.length })}
          </span>
        </header>

        {referrals.length === 0 ? (
          <article className="rounded-proph-lg border border-proph-divider-soft bg-proph-canvas p-8 text-center">
            <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("referrals.empty")}
            </p>
          </article>
        ) : (
          <ul className="divide-y divide-proph-divider-soft rounded-proph-lg border border-proph-hairline bg-proph-canvas">
            {referrals.map((referral) => (
              <ReferralRow key={referral.referredId} t={t} referral={referral} />
            ))}
          </ul>
        )}

        <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          {t("referrals.disclaimer")}
        </p>
      </div>
    </section>
  );
}

function ReferralRow({ t, referral }: { t: Copy; referral: AffiliateReferralListItem }) {
  return (
    <li className="grid grid-cols-1 items-center gap-3 p-5 md:grid-cols-[2fr_auto_auto_auto]">
      <div className="min-w-0">
        <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {t("referrals.fields.username")}
        </div>
        <div className="mt-1 truncate font-mono text-[17px] tracking-[-0.374px] text-proph-ink">
          {referral.maskedUsername}
        </div>
      </div>
      <div>
        <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {t("referrals.fields.date")}
        </div>
        <div className="mt-1 font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
          {epochDayToISO(referral.referredAtDay)}
        </div>
      </div>
      <div>
        <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {t("referrals.fields.status")}
        </div>
        <div className="mt-1">
          <StatusPill t={t} status={referral.status} />
        </div>
      </div>
      <div className="md:text-right">
        <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
          {t("referrals.fields.commission")}
        </div>
        <div className="mt-1 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink">
          {formatPoints(referral.cumulativeCommissionPoints)}
        </div>
      </div>
    </li>
  );
}

function StatusPill({ t, status }: { t: Copy; status: AffiliateReferralStatus }) {
  const tone = STATUS_TONE[status];
  const toneClass =
    tone === "positive"
      ? "border-proph-feedback-success/40 text-proph-feedback-success"
      : tone === "negative"
        ? "border-proph-feedback-error/40 text-proph-feedback-error"
        : tone === "warning"
          ? "border-proph-primary/40 text-proph-primary"
          : "border-proph-hairline text-proph-ink-muted-80";

  return (
    <span
      className={`inline-flex items-center rounded-proph-pill border px-3 py-1 font-text text-[12px] uppercase tracking-[-0.12px] ${toneClass}`}
    >
      {t(`referrals.statuses.${status}`)}
    </span>
  );
}

function NotMlmReminder({ t }: { t: Copy }) {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
          {t("notMlm.headline")}
        </h2>
        <p className="mt-3 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          {t("notMlm.body")}
        </p>
      </div>
    </section>
  );
}

const STATUS_TONE: Record<
  AffiliateReferralStatus,
  "positive" | "negative" | "warning" | "neutral"
> = {
  qualified: "positive",
  disqualified: "negative",
  pending: "neutral",
  pending_review: "warning",
  pending_eddreview: "warning",
};

const MS_PER_DAY = 86_400_000;

function epochDayToISO(day: number): string {
  const date = new Date(day * MS_PER_DAY);
  if (Number.isNaN(date.getTime())) return "—";
  const yyyy = date.getUTCFullYear();
  const mm = String(date.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(date.getUTCDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}
