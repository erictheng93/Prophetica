import type { Metadata } from "next";
import { TrendDown, TrendUp } from "@phosphor-icons/react/dist/ssr";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  legalColumn,
  MainNavLinks,
  marketsColumn,
  supportColumn,
} from "@/lib/site-nav";
import {
  categoryLabel,
  formatPoints,
  statusLabel,
} from "@/lib/mock-markets";
import {
  formatPointsSigned,
  listActivePositions,
  listSettledPositions,
  type ResolvedPosition,
  summarizePortfolio,
} from "@/lib/mock-portfolio";

export const metadata: Metadata = {
  title: "個人持倉 — prophetica",
  description: "查看你的進行中與已結算持倉，以及整體損益快照。",
  robots: { index: false, follow: false },
};

const sectionFooterColumns = [marketsColumn, legalColumn, supportColumn];

export default function PortfolioPage() {
  const summary = summarizePortfolio();
  const active = listActivePositions();
  const settled = listSettledPositions();

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        kicker="個人持倉"
        headline="你目前的對局組合。"
        tagline="進行中持倉以最後成交價即時計價，已結算持倉顯示實際損益。所有金額以站內點數表示，不可兌回現金。"
      />

      <DemoBanner />

      <SummaryCards summary={summary} />

      <PositionsSection title="進行中" subtitle="跟著市場波動的持倉，價格採用最後成交價。" positions={active} variant="active" />

      <PositionsSection title="已結算" subtitle="Oracle 已判定、結果已入帳。" positions={settled} variant="settled" />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function DemoBanner() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
        <p className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          <strong className="font-semibold text-proph-ink">示意數據。</strong>{" "}
          這些是 prophetica 團隊內部測試資料，幫你理解持倉頁長相。實際持倉須在你登入並完成 KYC 後顯示。
        </p>
      </div>
    </section>
  );
}

function SummaryCards({ summary }: { summary: ReturnType<typeof summarizePortfolio> }) {
  const cards = [
    {
      kicker: "進行中投入",
      label: "總成本",
      value: formatPoints(summary.totalCostPoints),
      tone: "neutral" as const,
    },
    {
      kicker: "進行中市值",
      label: "目前價值",
      value: formatPoints(summary.currentValuePoints),
      tone: "neutral" as const,
    },
    {
      kicker: "進行中未實現",
      label: "未實現損益",
      value: formatPointsSigned(summary.unrealizedPnlPoints),
      tone: pickTone(summary.unrealizedPnlPoints),
    },
    {
      kicker: "已結算",
      label: "累計已實現損益",
      value: formatPointsSigned(summary.realizedPnlPointsLifetime),
      tone: pickTone(summary.realizedPnlPointsLifetime),
    },
  ];

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          {cards.map((card) => (
            <article key={card.kicker} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {card.kicker}
              </div>
              <div
                className={
                  "mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px] " +
                  toneClass(card.tone)
                }
              >
                {card.value}
              </div>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                {card.label}
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function PositionsSection({
  title,
  subtitle,
  positions,
  variant,
}: {
  title: string;
  subtitle: string;
  positions: ResolvedPosition[];
  variant: "active" | "settled";
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8 flex items-baseline justify-between">
          <div>
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">{title}</h2>
            <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {subtitle}
            </p>
          </div>
          <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {positions.length} 筆
          </span>
        </header>

        {positions.length === 0 ? (
          <EmptyPlaceholder variant={variant} />
        ) : (
          <ul className="space-y-4">
            {positions.map((p) => (
              <PositionRow key={`${p.marketId}-${p.outcome}`} position={p} variant={variant} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function PositionRow({ position, variant }: { position: ResolvedPosition; variant: "active" | "settled" }) {
  const { market, outcome, shares, avgCostCents } = position;
  const sideClass = outcome === "YES" ? "text-proph-primary" : "text-proph-ink";

  const pnl =
    variant === "active" ? position.unrealizedPnlPoints : (position.realizedPnlPoints ?? 0);
  const pnlTone = pickTone(pnl);

  return (
    <li>
      <a
        href={`/markets/${market.id}`}
        className="block rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
      >
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
                {categoryLabel[market.category]} · {statusLabel[market.status]}
              </span>
            </div>
            <h3 className="mt-1 font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
              {market.title}
            </h3>
          </div>
          <span
            className={
              "shrink-0 rounded-proph-pill border px-3 py-1 font-display text-[14px] font-semibold tracking-[-0.224px] " +
              (outcome === "YES"
                ? "border-proph-primary text-proph-primary"
                : "border-proph-hairline text-proph-ink")
            }
          >
            {outcome}
          </span>
        </div>

        <dl className="mt-5 grid grid-cols-2 gap-4 border-t border-proph-divider-soft pt-4 md:grid-cols-4">
          <Field label="持有股數" value={shares.toLocaleString()} mono />
          <Field label="均價" value={`${avgCostCents}¢`} mono />
          <Field
            label={variant === "active" ? "目前價值" : "結算入帳"}
            value={formatPoints(
              variant === "active" ? position.currentValuePoints : position.totalCostPoints + (position.realizedPnlPoints ?? 0),
            )}
            mono
          />
          <Field
            label={variant === "active" ? "未實現損益" : "已實現損益"}
            value={formatPointsSigned(pnl)}
            mono
            tone={pnlTone}
            extra={
              variant === "active"
                ? `${position.unrealizedPnlPct >= 0 ? "+" : "−"}${Math.abs(position.unrealizedPnlPct).toFixed(1)}%`
                : undefined
            }
          />
        </dl>
      </a>
    </li>
  );
}

function Field({
  label,
  value,
  mono = false,
  tone = "neutral",
  extra,
}: {
  label: string;
  value: string;
  mono?: boolean;
  tone?: "neutral" | "positive" | "negative";
  extra?: string;
}) {
  return (
    <div>
      <dt className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd
        className={
          "mt-1 inline-flex items-center gap-1 " +
          (mono ? "font-mono tabular-nums text-[17px] tracking-[-0.374px] " : "font-text text-[17px] tracking-[-0.374px] ") +
          toneClass(tone)
        }
      >
        {tone === "positive" ? (
          <TrendUp weight="bold" className="h-4 w-4 shrink-0" aria-hidden="true" />
        ) : tone === "negative" ? (
          <TrendDown weight="bold" className="h-4 w-4 shrink-0" aria-hidden="true" />
        ) : null}
        <span>{value}</span>
        {extra ? (
          <span className="ml-1 font-text text-[12px] tracking-[-0.12px]">{extra}</span>
        ) : null}
      </dd>
    </div>
  );
}

function EmptyPlaceholder({ variant }: { variant: "active" | "settled" }) {
  return (
    <article className="rounded-proph-lg border border-proph-divider-soft bg-proph-canvas-parchment p-8 text-center">
      <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
        {variant === "active" ? "目前沒有進行中持倉。" : "尚未有任何結算紀錄。"}
      </p>
      <ProphLinkButton variant="primary" href="/markets" className="mt-5">
        瀏覽市場
      </ProphLinkButton>
    </article>
  );
}

type Tone = "neutral" | "positive" | "negative";

function pickTone(value: number): Tone {
  if (value > 0) return "positive";
  if (value < 0) return "negative";
  return "neutral";
}

function toneClass(tone: Tone): string {
  switch (tone) {
    case "positive":
      return "text-proph-feedback-success";
    case "negative":
      return "text-proph-feedback-error";
    default:
      return "text-proph-ink";
  }
}
