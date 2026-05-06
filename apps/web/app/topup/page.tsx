import type { Metadata } from "next";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  legalColumn,
  MainNavLinks,
  responsibleGamingColumn,
  supportColumn,
} from "@/lib/site-nav";

export const metadata: Metadata = {
  title: "點數儲值 — prophetica",
  description: "為帳戶儲值站內點數。1 點 = TWD 1。儲值受月儲值上限保護。",
  robots: { index: false, follow: false },
};

const sectionFooterColumns = [responsibleGamingColumn, legalColumn, supportColumn];

const presetAmounts = [100, 500, 1_000, 5_000, 10_000];

interface ChannelCard {
  name: string;
  status: "active" | "backup" | "phase-2";
  fee: string;
  payoutEta: string;
  body: string;
}

// SPEC-016 §3.1 channel matrix.
const channels: ChannelCard[] = [
  {
    name: "綠界 ECPay",
    status: "active",
    fee: "1.5–2.5%",
    payoutEta: "T+3 至 T+7",
    body: "信用卡 / ATM 轉帳 / 超商代碼。台灣首選通路；MCC 7995 / 7993 / 7800 已通過業務驗證。",
  },
  {
    name: "藍新 NewebPay",
    status: "backup",
    fee: "1.5–2.5%",
    payoutEta: "T+5",
    body: "綠界備援。MVP + 30 天啟用，並列為主要通路提供 fallback 切換能力。",
  },
  {
    name: "Stripe（國際）",
    status: "phase-2",
    fee: "2.9% + USD 0.30",
    payoutEta: "T+2",
    body: "Op Co. 國際版本啟用後開放；目前處於 SPEC-016 §7 Phase 2 範圍，無啟用時程。",
  },
  {
    name: "區域通路（VN / PH / MY）",
    status: "phase-2",
    fee: "—",
    payoutEta: "—",
    body: "MoMo / VNPay / PayMongo 等。執照地律師確認各國 MCC 接受度後評估啟用。",
  },
];

// Mock player state — replaced by user-service / wallet-ledger snapshot.
const mockState = {
  currentBalance: 12_500,
  monthlyDeposited: 8_400,
  monthlyLimit: 30_000,
  kycTier: 1,
};

const formatPoints = (n: number): string => `${n.toLocaleString("zh-Hant")} 點`;
const formatTwd = (n: number): string => `TWD ${n.toLocaleString("zh-Hant")}`;

export default function TopUpPage() {
  const { currentBalance, monthlyDeposited, monthlyLimit, kycTier } = mockState;
  const usagePct = Math.min(100, (monthlyDeposited / monthlyLimit) * 100);
  const remainingPoints = Math.max(0, monthlyLimit - monthlyDeposited);
  // SPEC-006 §7.2 thresholds.
  const nearLimit = usagePct >= 80 && usagePct < 100;
  const atLimit = usagePct >= 100;

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        kicker="點數儲值"
        headline="把現金換成站內點數，僅限站內使用。"
        tagline="1 點 = TWD 1（內部記帳基準）。儲值受你設定的月儲值上限保護；達上限將直接拒絕儲值。"
      />

      <DemoBanner />

      <BalanceBlock
        currentBalance={currentBalance}
        monthlyDeposited={monthlyDeposited}
        monthlyLimit={monthlyLimit}
        remainingPoints={remainingPoints}
        usagePct={usagePct}
        nearLimit={nearLimit}
        atLimit={atLimit}
        kycTier={kycTier}
      />

      <AmountSection atLimit={atLimit} remainingPoints={remainingPoints} />

      <ChannelSection />

      <ConfirmSection atLimit={atLimit} />

      <RGReminder />

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
          實際餘額與月儲值用量需登入後顯示。儲值通路串接由 SPEC-016 後端就緒後啟用。
        </p>
      </div>
    </section>
  );
}

interface BalanceBlockProps {
  currentBalance: number;
  monthlyDeposited: number;
  monthlyLimit: number;
  remainingPoints: number;
  usagePct: number;
  nearLimit: boolean;
  atLimit: boolean;
  kycTier: number;
}

function BalanceBlock(props: BalanceBlockProps) {
  const { currentBalance, monthlyDeposited, monthlyLimit, remainingPoints, usagePct, nearLimit, atLimit, kycTier } = props;
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              目前餘額
            </div>
            <div className="mt-2 font-display text-[56px] font-semibold leading-none tracking-[-0.28px]">
              {formatPoints(currentBalance)}
            </div>
            <div className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              KYC Tier {kycTier} · 單筆上限{" "}
              {kycTier === 1 ? "TWD 5,000" : kycTier === 2 ? "—" : "個案核定"}
            </div>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              本月儲值用量
            </div>
            <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
              {formatPoints(monthlyDeposited)} / {formatPoints(monthlyLimit)}
            </div>
            <div className="mt-4 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
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
                aria-label={`月儲值用量 ${usagePct.toFixed(0)}%`}
              />
            </div>
            <div className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              本月還可儲值 {formatPoints(remainingPoints)}
            </div>

            {atLimit ? (
              <p className="mt-4 rounded-proph-md border border-proph-feedback-error bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-error">
                你已達本月儲值上限。下個月 1 日重置；或前往
                <a href="/responsible-gaming/limits" className="ml-1 underline">
                  限額設定
                </a>
                查看（提高需 24 小時冷卻期）。
              </p>
            ) : nearLimit ? (
              <p className="mt-4 rounded-proph-md border border-proph-feedback-warning bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-warning">
                你已使用月儲值上限的 {usagePct.toFixed(0)}%。請考慮先暫停儲值，本月不再加碼。
              </p>
            ) : null}
          </article>
        </div>
      </div>
    </section>
  );
}

function AmountSection({
  atLimit,
  remainingPoints,
}: {
  atLimit: boolean;
  remainingPoints: number;
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">儲值金額</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            選擇預設金額，或自訂。1 點 = TWD 1，本次最多可儲值 {formatPoints(remainingPoints)}。
          </p>
        </header>

        <form action="#" aria-disabled="true" className="space-y-6">
          <fieldset className={"space-y-3 " + (atLimit ? "opacity-50 pointer-events-none" : "")}>
            <legend className="sr-only">選擇儲值金額</legend>
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
                    點 / {formatTwd(amt)}
                  </span>
                </label>
              ))}
            </div>

            <label className="flex flex-col gap-2 rounded-proph-md border border-proph-hairline bg-proph-canvas p-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment">
              <span className="flex items-center gap-3">
                <input type="radio" name="amount" value="custom" className="h-4 w-4 accent-proph-primary" />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">自訂金額</span>
              </span>
              <input
                type="number"
                inputMode="numeric"
                min={1}
                max={remainingPoints || undefined}
                placeholder={`例如：2,500（最多 ${remainingPoints.toLocaleString()}）`}
                className="ml-7 w-full max-w-xs rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                aria-label="自訂儲值金額"
              />
            </label>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

function ChannelSection() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">選擇支付通路</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            台灣首發走綠界（主）/ 藍新（備援）。國際與區域通路待 Op Co. 啟用後評估。
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {channels.map((c) => {
            const disabled = c.status === "phase-2";
            return (
              <li
                key={c.name}
                className={
                  "flex flex-col rounded-proph-lg border bg-proph-canvas p-6 " +
                  (disabled ? "border-proph-divider-soft opacity-60" : "border-proph-hairline")
                }
              >
                <div className="flex items-baseline justify-between gap-3">
                  <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{c.name}</h3>
                  <ChannelStatusPill status={c.status} />
                </div>
                <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {c.body}
                </p>
                <dl className="mt-auto grid grid-cols-2 gap-3 border-t border-proph-divider-soft pt-4">
                  <div>
                    <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      手續費
                    </dt>
                    <dd className="mt-1 font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                      {c.fee}
                    </dd>
                  </div>
                  <div>
                    <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      平台收款
                    </dt>
                    <dd className="mt-1 font-mono text-[14px] tabular-nums tracking-[-0.224px] text-proph-ink-muted-80">
                      {c.payoutEta}
                    </dd>
                  </div>
                </dl>
              </li>
            );
          })}
        </ul>

        <p className="mt-6 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
          手續費由平台吸收，不會額外向你收取。通路結算時程影響的是平台對帳節奏，與你的儲值入帳速度無關（通常 5 分鐘內入帳）。
        </p>
      </div>
    </section>
  );
}

function ChannelStatusPill({ status }: { status: ChannelCard["status"] }) {
  const config = {
    active: { label: "Tier-1", class: "border-proph-primary text-proph-primary" },
    backup: { label: "Tier-2", class: "border-proph-feedback-warning text-proph-feedback-warning" },
    "phase-2": { label: "Phase 2", class: "border-proph-divider-soft text-proph-ink-muted-48" },
  } as const;
  const c = config[status];
  return (
    <span
      className={`shrink-0 rounded-proph-pill border px-3 py-0.5 font-text text-[12px] uppercase tracking-[-0.12px] ${c.class}`}
    >
      {c.label}
    </span>
  );
}

function ConfirmSection({ atLimit }: { atLimit: boolean }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            確認摘要
          </h2>
          <p className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            送出後將跳轉至支付通路頁面完成付款。付款成功後 5 分鐘內點數入帳。
          </p>

          <dl className="mt-6 space-y-3 border-t border-proph-divider-soft pt-4">
            <Row label="儲值金額" value="1,000 點 / TWD 1,000" />
            <Row label="支付通路" value="綠界 ECPay（信用卡）" />
            <Row label="手續費" value="平台吸收" hint />
            <Row label="預估入帳時間" value="5 分鐘內" />
          </dl>

          <label className="mt-6 flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
            <span>
              我確認儲值取得的點數
              <strong className="font-semibold text-proph-ink"> 不可兌回 </strong>
              現金、虛擬貨幣或實體商品，僅限站內娛樂使用。
            </span>
          </label>

          <div className="mt-6 flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              支付完成將回到 prophetica；中途可取消但不退款部分流程已扣款情況。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {atLimit ? "已達月上限，無法儲值" : "前往支付（後端待接）"}
            </button>
          </div>
        </article>
      </div>
    </section>
  );
}

function Row({ label, value, hint }: { label: string; value: string; hint?: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <dt className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">{label}</dt>
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

function RGReminder() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          請務必記得
        </span>
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
          月儲值上限是你給自己的保護線。如果儲值節奏失控，先按下暫停。
        </h2>
        <div className="mt-6 flex flex-wrap items-center gap-4">
          <ProphLinkButton variant="primary" href="/responsible-gaming/cooldown">
            啟動冷靜期
          </ProphLinkButton>
          <a
            href="tel:1925"
            className="font-mono text-[21px] font-semibold tabular-nums text-proph-primary-on-dark hover:underline"
          >
            1925{" "}
            <span className="font-text text-[14px] tracking-[-0.224px] text-proph-body-muted">
              安心專線 · 24 小時
            </span>
          </a>
        </div>
      </div>
    </section>
  );
}
