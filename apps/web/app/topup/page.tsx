import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import {
  ProphFooter,
  ProphLinkButton,
  ProphSubNav,
  ProphTile,
} from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";
import { fetchWalletOverview } from "@/lib/api-gateway";
import { fetchPaymentsRealMoneyStatus } from "@/lib/payments-api";
import { safeReturnTo } from "@/lib/safe-return-to";
import { getUpstreamSessionToken } from "@/lib/session";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("topup.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

const presetAmounts = [100, 500, 1_000, 5_000, 10_000];
type ChannelStatus = "active" | "backup" | "phase-2";
type ChannelKey = "ecpay" | "newebpay" | "stripe" | "regional";

const CHANNEL_KEYS: { key: ChannelKey; status: ChannelStatus }[] = [
  { key: "ecpay", status: "active" },
  { key: "newebpay", status: "backup" },
  { key: "stripe", status: "phase-2" },
  { key: "regional", status: "phase-2" },
];

const STATUS_TIER_KEY: Record<ChannelStatus, "active" | "backup" | "phase2"> = {
  active: "active",
  backup: "backup",
  "phase-2": "phase2",
};

const mockState = {
  monthlyDeposited: 8_400,
  monthlyLimit: 30_000,
  kycTier: 1,
};

const formatTwd = (n: number): string => `TWD ${n.toLocaleString("zh-Hant")}`;

type TopupCopy = Awaited<ReturnType<typeof getTranslations<"topup">>>;

interface TopUpPageProps {
  searchParams?: Promise<{
    return_to?: string | string[];
  }>;
}

export default async function TopUpPage({ searchParams }: TopUpPageProps) {
  const params = await searchParams;
  const rawReturnTo = Array.isArray(params?.return_to)
    ? params?.return_to[0]
    : params?.return_to;
  const returnTo = safeReturnTo(rawReturnTo);
  const upstreamSessionToken = await getUpstreamSessionToken();
  const [t, tFmt, sectionFooterColumns, walletOverview, paymentsStatus] =
    await Promise.all([
      getTranslations("topup"),
      getTranslations("common.format"),
      getFooterColumns(["responsibleGaming", "legal", "support"]),
      fetchWalletOverview({ bearerToken: upstreamSessionToken ?? undefined }),
      fetchPaymentsRealMoneyStatus(),
    ]);
  const formatPoints = (n: number) =>
    tFmt("points", { n: n.toLocaleString("zh-Hant") });
  const { monthlyDeposited, monthlyLimit, kycTier } = mockState;
  const currentBalance = walletOverview.balance.availablePoints;
  const usagePct = Math.min(100, (monthlyDeposited / monthlyLimit) * 100);
  const remainingPoints = Math.max(0, monthlyLimit - monthlyDeposited);
  const nearLimit = usagePct >= 80 && usagePct < 100;
  const atLimit = usagePct >= 100;

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<AuthCta />}
      />

      <ProphTile
        surface="light"
        glow="gold"
        gridPattern
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
          <p className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <strong className="font-semibold text-proph-ink">
              {t("demoBanner.label")}
            </strong>{" "}
            {t("demoBanner.body")}
          </p>
        </div>
      </section>

      <BalanceBlock
        t={t}
        currentBalance={currentBalance}
        monthlyDeposited={monthlyDeposited}
        monthlyLimit={monthlyLimit}
        remainingPoints={remainingPoints}
        usagePct={usagePct}
        nearLimit={nearLimit}
        atLimit={atLimit}
        kycTier={kycTier}
        walletSource={walletOverview.source}
        formatPoints={formatPoints}
      />

      {paymentsStatus.realMoneyEnabled ? (
        <>
          <AmountSection
            t={t}
            atLimit={atLimit}
            remainingPoints={remainingPoints}
            formatPoints={formatPoints}
          />
          <ChannelSection t={t} />
          <ConfirmSection t={t} atLimit={atLimit} />
        </>
      ) : (
        <ServiceAssistedTopup
          t={t}
          statusSource={paymentsStatus.source}
          returnTo={returnTo}
        />
      )}
      <RGReminder t={t} />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function ServiceAssistedTopup({
  t,
  statusSource,
  returnTo,
}: {
  t: TopupCopy;
  statusSource: string;
  returnTo: string;
}) {
  const supportHref = `/support/contact?topic=topup&return_to=${encodeURIComponent(returnTo)}`;

  return (
    <section
      className="bg-proph-canvas"
      aria-labelledby="topup-assistance-heading"
    >
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-8 shadow-proph-card">
          <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
            {t("mvpAssistance.label")}
          </div>
          <h2
            id="topup-assistance-heading"
            className="mt-3 font-display text-[28px] font-semibold leading-[1.1] tracking-[0.196px] sm:text-[34px] md:text-[40px]"
          >
            {t("mvpAssistance.heading")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("mvpAssistance.body")}
          </p>
          {statusSource !== "api-gateway" ? (
            <p className="mt-5 rounded-proph-md border border-proph-feedback-warning bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-warning">
              {t("mvpAssistance.flagUnavailable")}
            </p>
          ) : null}
          <div className="mt-6 flex flex-wrap items-center gap-3">
            <ProphLinkButton variant="primary" href={supportHref}>
              {t("mvpAssistance.ctaSupport")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/support/faq#topup">
              {t("mvpAssistance.ctaFaq")}
            </ProphLinkButton>
          </div>
        </article>
      </div>
    </section>
  );
}

interface BalanceBlockProps {
  t: TopupCopy;
  currentBalance: number;
  monthlyDeposited: number;
  monthlyLimit: number;
  remainingPoints: number;
  usagePct: number;
  nearLimit: boolean;
  atLimit: boolean;
  kycTier: number;
  walletSource: string;
  formatPoints: (n: number) => string;
}

function BalanceBlock(props: BalanceBlockProps) {
  const {
    t,
    currentBalance,
    monthlyDeposited,
    monthlyLimit,
    remainingPoints,
    usagePct,
    nearLimit,
    atLimit,
    kycTier,
    walletSource,
    formatPoints,
  } = props;

  const tierCap =
    kycTier === 1
      ? t("balance.kycCapTier1")
      : kycTier === 2
        ? t("balance.kycCapTier2")
        : t("balance.kycCapTier3");

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="relative overflow-hidden rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
            <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              {t("balance.currentLabel")}
            </div>
            <div className="mt-2 font-display text-[28px] font-semibold leading-none tracking-[-0.28px] sm:text-[34px] md:text-[40px] lg:text-[56px]">
              {formatPoints(currentBalance)}
            </div>
            <div className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("balance.kycLine", { tier: kycTier, cap: tierCap })}
            </div>
            {walletSource !== "api-gateway" ? (
              <p className="mt-4 rounded-proph-md border border-proph-feedback-warning bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-warning">
                {t("balance.gatewayUnreachable")}
              </p>
            ) : null}
          </article>

          <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
            <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              {t("balance.monthlyLabel")}
            </div>
            <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
              {formatPoints(monthlyDeposited)} / {formatPoints(monthlyLimit)}
            </div>
            <div
              className="mt-4 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft"
              role="progressbar"
              aria-label={t("balance.progressLabel")}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(usagePct)}
            >
              <div
                className={
                  "h-full " +
                  (atLimit
                    ? "bg-proph-feedback-error"
                    : nearLimit
                      ? "bg-proph-feedback-warning"
                      : "bg-proph-primary")
                }
                style={{ width: `${usagePct}%` }}
              />
            </div>
            <div className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("balance.remaining", {
                amount: formatPoints(remainingPoints),
              })}
            </div>

            {atLimit ? (
              <p className="mt-4 rounded-proph-md border border-proph-feedback-error bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-error">
                {t.rich("balance.atLimit", {
                  limitsLink: (chunks) => (
                    <a
                      href="/responsible-gaming/limits"
                      className="ml-1 underline"
                    >
                      {chunks}
                    </a>
                  ),
                })}
              </p>
            ) : nearLimit ? (
              <p className="mt-4 rounded-proph-md border border-proph-feedback-warning bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-warning">
                {t("balance.nearLimit", { pct: usagePct.toFixed(0) })}
              </p>
            ) : null}
          </article>
        </div>
      </div>
    </section>
  );
}

function AmountSection({
  t,
  atLimit,
  remainingPoints,
  formatPoints,
}: {
  t: TopupCopy;
  atLimit: boolean;
  remainingPoints: number;
  formatPoints: (n: number) => string;
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("amount.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("amount.intro", { remaining: formatPoints(remainingPoints) })}
          </p>
        </header>

        <form action="#" className="space-y-6">
          <fieldset
            className={
              "space-y-3 " + (atLimit ? "opacity-50 pointer-events-none" : "")
            }
          >
            <legend className="sr-only">{t("amount.heading")}</legend>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
              {presetAmounts.map((amt) => (
                <label
                  key={amt}
                  className="cursor-pointer rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 text-center has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <input
                    type="radio"
                    name="amount"
                    value={amt}
                    defaultChecked={amt === 1_000}
                    className="sr-only"
                    disabled={amt > remainingPoints}
                  />
                  <span className="block font-display text-[28px] font-semibold leading-none tracking-[-0.374px] text-proph-ink">
                    {amt.toLocaleString()}
                  </span>
                  <span className="mt-1 block font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("amount.presetPoints", { twd: formatTwd(amt) })}
                  </span>
                </label>
              ))}
            </div>

            <label className="flex flex-col gap-2 rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment">
              <span className="flex items-center gap-3">
                <input
                  type="radio"
                  name="amount"
                  value="custom"
                  className="h-4 w-4 accent-proph-primary"
                />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t("amount.customLabel")}
                </span>
              </span>
              <input
                type="number"
                inputMode="numeric"
                min={1}
                max={remainingPoints || undefined}
                placeholder={t("amount.customPlaceholder", {
                  max: remainingPoints.toLocaleString(),
                })}
                className="ml-7 w-full min-h-11 max-w-xs rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                aria-label={t("amount.customAriaLabel")}
              />
            </label>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

function ChannelSection({ t }: { t: TopupCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("channels.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("channels.intro")}
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {CHANNEL_KEYS.map(({ key, status }) => {
            const disabled = status === "phase-2";
            return (
              <li
                key={key}
                className={
                  "flex flex-col rounded-proph-xl border p-6 transition-all duration-200 " +
                  (disabled
                    ? "border-proph-divider-soft bg-proph-canvas-parchment opacity-70"
                    : "border-proph-hairline/80 bg-proph-canvas shadow-proph-card hover:-translate-y-0.5 hover:border-proph-primary/40")
                }
              >
                <div className="flex items-baseline justify-between gap-3">
                  <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {t(`channels.items.${key}.name`)}
                  </h3>
                  <ChannelStatusPill t={t} status={status} />
                </div>
                <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {t(`channels.items.${key}.body`)}
                </p>
                <dl className="mt-auto grid grid-cols-2 gap-3 border-t border-proph-divider-soft pt-4">
                  <div>
                    <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      {t("channels.feeLabel")}
                    </dt>
                    <dd className="mt-1 font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                      {t(`channels.items.${key}.fee`)}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      {t("channels.payoutLabel")}
                    </dt>
                    <dd className="mt-1 font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                      {t(`channels.items.${key}.payoutEta`)}
                    </dd>
                  </div>
                </dl>
              </li>
            );
          })}
        </ul>

        <p className="mt-6 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
          {t("channels.footer")}
        </p>
      </div>
    </section>
  );
}

function ChannelStatusPill({
  t,
  status,
}: {
  t: TopupCopy;
  status: ChannelStatus;
}) {
  const tierKey = STATUS_TIER_KEY[status];
  const classMap: Record<ChannelStatus, string> = {
    active: "border-proph-primary text-proph-primary",
    backup: "border-proph-feedback-warning text-proph-feedback-warning",
    "phase-2": "border-proph-divider-soft text-proph-ink-muted-48",
  };
  return (
    <span
      className={`shrink-0 rounded-proph-pill border px-3 py-0.5 font-text text-[12px] uppercase tracking-[-0.12px] ${classMap[status]}`}
    >
      {t(`channels.tier.${tierKey}`)}
    </span>
  );
}

function ConfirmSection({ t, atLimit }: { t: TopupCopy; atLimit: boolean }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-8 shadow-proph-card">
          <h2 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            {t("confirm.heading")}
          </h2>
          <p className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("confirm.intro")}
          </p>

          <dl className="mt-6 space-y-3 border-t border-proph-divider-soft pt-4">
            <Row
              label={t("confirm.rows.amount.label")}
              value={t("confirm.rows.amount.value")}
            />
            <Row
              label={t("confirm.rows.channel.label")}
              value={t("confirm.rows.channel.value")}
            />
            <Row
              label={t("confirm.rows.fee.label")}
              value={t("confirm.rows.fee.value")}
              hint
            />
            <Row
              label={t("confirm.rows.eta.label")}
              value={t("confirm.rows.eta.value")}
            />
          </dl>

          <label className="mt-6 flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <input
              type="checkbox"
              required
              className="mt-1 h-4 w-4 accent-proph-primary"
            />
            <span>
              {t.rich("confirm.consent", {
                strong: (chunks) => (
                  <strong className="font-semibold text-proph-ink">
                    {" "}
                    {chunks}{" "}
                  </strong>
                ),
              })}
            </span>
          </label>

          <div className="mt-6 flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("confirm.footnote")}
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {atLimit ? t("confirm.ctaAtLimit") : t("confirm.ctaPay")}
            </button>
          </div>
        </article>
      </div>
    </section>
  );
}

function Row({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: boolean;
}) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <dt className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd
        className={
          "font-mono text-[17px] tabular-nums tracking-[-0.374px] " +
          (hint ? "text-proph-ink-muted-80" : "text-proph-ink")
        }
      >
        {value}
      </dd>
    </div>
  );
}

function RGReminder({ t }: { t: TopupCopy }) {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
          {t("rgReminder.headline")}
        </h2>
        <div className="mt-6 flex flex-wrap items-center gap-4">
          <ProphLinkButton
            variant="primary"
            href="/responsible-gaming/cooldown"
          >
            {t("rgReminder.ctaCooldown")}
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}
