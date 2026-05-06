import type { Metadata } from "next";
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

export const metadata: Metadata = {
  title: "代理計畫 — prophetica",
  description: "單層介紹費 + 12 個月交易佣金。不是 MLM，不是多層代理。所有佣金可被稽核、稅務透明。",
};

const sectionFooterColumns = [marketsColumn, legalColumn, supportColumn];

function HeroBlock() {
  return (
    <ProphTile
      surface="light"
      kicker="代理計畫"
      headline={
        <span className="font-display text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
          單層介紹費 + 12 個月交易佣金。
        </span>
      }
      tagline="prophetica 不採傳統多層代理（MLM）架構。介紹一位玩家，你拿一次註冊獎金與 12 個月內的交易手續費分潤；不會有「下線的下線」結構。"
      actions={<ProphLinkButton variant="primary" href="/support/contact?topic=affiliate">申請成為代理</ProphLinkButton>}
    />
  );
}

function CommissionBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">兩種佣金</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            不重複計算、不疊加層級。所有佣金來自平台手續費，與玩家輸贏無關。
          </p>
        </header>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              一次性
            </div>
            <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              註冊獎金
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              被推薦人完成 KYC Tier 1 + 首次儲值 ≥ TWD 500 → 你拿 TWD 200 等值點數。
              一個被推薦人只計算一次，避免重複註冊套利。
            </p>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              持續 12 個月
            </div>
            <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              交易佣金
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              被推薦人下單後，平台從手續費中抽 20% 給你。期限為被推薦人註冊後 12 個月，到期自動停止。
              重點：佣金來自手續費，不是玩家本金或輸贏。
            </p>
          </article>
        </div>
      </div>
    </section>
  );
}

function WorkedExampleBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">具體例子</h2>
        </header>

        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
          <p className="font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            假設你介紹了一位玩家，他註冊後 6 個月內：
          </p>
          <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            <li>· 完成 KYC Tier 1 + 首次儲值 TWD 1,000</li>
            <li>· 6 個月內累計成交量 TWD 50,000，平台收取手續費約 TWD 1,000（雙邊各 1%）</li>
          </ul>

          <dl className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-3">
            <div className="rounded-proph-md border border-proph-feedback-success/30 bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-success">
                註冊獎金
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-ink">
                TWD 200
              </dd>
              <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                一次性，KYC + 首儲後入帳
              </p>
            </div>
            <div className="rounded-proph-md border border-proph-feedback-success/30 bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-success">
                交易佣金 6 個月
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-ink">
                TWD 200
              </dd>
              <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                1,000 × 20% = 200，每月入帳
              </p>
            </div>
            <div className="rounded-proph-md border border-proph-hairline bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                合計
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-ink">
                TWD 400
              </dd>
              <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                依被推薦人活躍度浮動；上限為其 12 個月手續費 × 20%
              </p>
            </div>
          </dl>

          <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
            被推薦人不知道你拿了多少 — 這是隱私設計。你也看不到對方的詳細交易，只看到聚合的「本月成交量 / 你的佣金」。
          </p>
        </article>
      </div>
    </section>
  );
}

function ReversalsBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">反沖規則</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            佣金不是「保證收益」。下列情境會反沖已發放的佣金，避免誘因被濫用。
          </p>
        </header>

        <ul className="space-y-3">
          {[
            {
              t: "被推薦人 chargeback / 退款",
              b: "對應交易的佣金反沖。如果代理帳戶餘額不足，從未來佣金扣抵。",
            },
            {
              t: "被推薦人觸發洗售 / 多帳號偵測",
              b: "佣金凍結至調查結束。經 Compliance 確認違規 → 全額反沖 + 代理帳戶 review。",
            },
            {
              t: "代理本人觸發異常",
              b: "你自己的帳戶被標記 RG 高風險或 KYC 異常 → 佣金凍結至釐清。期間既有佣金照常計算但暫不發放。",
            },
            {
              t: "自我推薦",
              b: "用同一裝置 / 同一 KYC hash 推薦自己 → 取消代理資格、所有佣金反沖、永久不再發放。",
            },
          ].map((item) => (
            <li key={item.t} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.t}</h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.b}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function TaxBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">稅務處理</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            代理佣金為應稅所得。我們按執照地與你的居住地法規處理扣繳；不會讓你「以為是免稅獎金」。
          </p>
        </header>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">個人代理</h3>
            <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              <li>· 台灣居住者：依執行業務所得 10% 預扣</li>
              <li>· 年度 &gt; TWD 200,000 → 開立扣繳憑單</li>
              <li>· 跨境居住者依雙邊租稅協定處理</li>
            </ul>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">公司代理</h3>
            <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              <li>· 須提供統一編號 + 公司登記文件</li>
              <li>· 平台依發票金額入帳，不預扣</li>
              <li>· 跨境付款依執照地會計報表</li>
            </ul>
          </article>
        </div>

        <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
          稅務細節請以執照地與你的所在地當期法規為準。我們會在每年 1 月寄出前一年的扣繳憑單。
        </p>
      </div>
    </section>
  );
}

function NotMlmBlock() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          法律邊界
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          這不是多層次傳銷（MLM）。
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          我們刻意避開「下線的下線也分潤」結構。介紹是一層，沒有第二層。
          如果你看到任何聲稱「prophetica 多層代理 / 鑽石經理 / 招募下線」之類的廣告，那是詐騙，不是我們。
        </p>
        <ul className="mt-6 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          <li>· 沒有「招募費」、沒有「升級階級」、沒有「保證收益」</li>
          <li>· 佣金來自平台手續費，不是被推薦人本金</li>
          <li>· 12 個月期限到自動停止，沒有「終身分潤」</li>
        </ul>
      </div>
    </section>
  );
}

export default function AffiliatePage() {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />
      <HeroBlock />
      <CommissionBlock />
      <WorkedExampleBlock />
      <ReversalsBlock />
      <TaxBlock />
      <NotMlmBlock />

      <ProphTile
        surface="light"
        kicker="想開始？"
        headline="申請代理資格只需登入後填寫一份表單。"
        tagline="申請後由 Compliance 簡單審核（KYC 通過 + 無違規紀錄即可）。通過後取得專屬推薦連結。"
        actions={<ProphLinkButton variant="primary" href="/support/contact?topic=affiliate">申請代理</ProphLinkButton>}
      />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
