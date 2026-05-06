import type { Metadata } from "next";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  legalColumn,
  MainNavLinks,
  marketsColumn,
  responsibleGamingColumn,
  supportColumn,
} from "@/lib/site-nav";

export const metadata: Metadata = {
  title: "玩法說明 — prophetica",
  description: "從註冊到結算的完整玩家流程：4 步驟入場、撮合機制、cent 價格、Oracle 結算與申訴窗口。",
};

const sectionFooterColumns = [marketsColumn, responsibleGamingColumn, legalColumn, supportColumn];

interface PrincipleCard {
  kicker: string;
  title: string;
  body: string;
}

// SPEC-001 §1.1 三原則 in player-facing language.
const principles: PrincipleCard[] = [
  {
    kicker: "原則一",
    title: "點數不可兌回。",
    body: "儲值取得的點數僅限站內使用，永遠不可兌回現金、虛擬貨幣或實體商品。這不是限制，是 prophetica 的法律邊界。",
  },
  {
    kicker: "原則二",
    title: "平台不下場。",
    body: "prophetica 不持倉、不對賭、不從你的輸贏中獲利。我們只收手續費，幫玩家彼此撮合。",
  },
  {
    kicker: "原則三",
    title: "全程稽核留痕。",
    body: "所有金流、撮合、結算、判定都寫入不可竄改的事件流，保存 7 年以上，可被監管單位稽核。",
  },
];

interface Step {
  num: string;
  title: string;
  body: string;
  href?: string;
  cta?: string;
}

const startSteps: Step[] = [
  {
    num: "01",
    title: "註冊帳戶",
    body: "Email + 手機 OTP + 生日（≥18 歲）。10 秒內完成基礎驗證 Tier 1，無需上傳證件影像。",
    href: "/auth/sign-in",
    cta: "前往註冊",
  },
  {
    num: "02",
    title: "設定責任博弈",
    body: "月儲值上限是必設的保護線（TWD 1k / 5k / 30k / 100k 擇一）。可選擇加上下注上限、損失上限與冷靜期觸發點。",
    href: "/responsible-gaming/limits",
    cta: "了解限額",
  },
  {
    num: "03",
    title: "儲值點數",
    body: "依執照地的支付通路完成儲值，點數通常 5 分鐘內入帳。1 點 = TWD 1（內部記帳基準）。",
    href: "/topup",
    cta: "儲值說明",
  },
  {
    num: "04",
    title: "瀏覽市場、下單",
    body: "選擇你有把握的市場，買 YES 或 NO 一股，等市場結算。每股結算為 100 點（贏）或 0 點（輸）。",
    href: "/markets",
    cta: "查看市場",
  },
];

function Hero() {
  return (
    <ProphTile
      surface="light"
      kicker="玩法說明"
      headline={
        <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
          從註冊到結算，5 分鐘看懂。
        </span>
      }
      tagline="prophetica 是趣味預測娛樂遊戲。你用站內點數對未公開事件結果表態，與其他玩家對局。我們做撮合與結算，不下場。"
      actions={
        <>
          <ProphLinkButton variant="primary" href="/auth/sign-in">立即註冊</ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/markets">先看市場</ProphLinkButton>
        </>
      }
    />
  );
}

function PrinciplesBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">三條法律邊界</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            這些不是「我們的承諾」，是 prophetica 的設計約束。違反任一條等同關站，創辦人負刑責。
            知道這三條，才知道 prophetica 是什麼、不是什麼。
          </p>
        </header>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {principles.map((p) => (
            <article
              key={p.kicker}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {p.kicker}
              </div>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {p.title}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {p.body}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function StartStepsBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">入場 4 步驟</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            前兩步是法律必經（年齡 + 責任博弈），第三第四步才是玩法相關。
          </p>
        </header>
        <ol className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {startSteps.map((s) => (
            <li
              key={s.num}
              className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-primary">
                  {s.num}
                </span>
                <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                  {s.title}
                </h3>
              </div>
              <p className="mt-3 flex-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {s.body}
              </p>
              {s.href ? (
                <a
                  href={s.href}
                  className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary underline"
                >
                  {s.cta} →
                </a>
              ) : null}
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function MatchingBlock() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          撮合機制
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          每一筆對局，都是玩家對玩家。
        </h2>
        <p className="mt-4 max-w-3xl font-display text-[24px] font-light leading-[1.5] text-proph-body-muted">
          當你買 YES 一股，必有另一個玩家賣出（或同時有人買 NO 一股）。我們把訂單放進
          <strong className="font-semibold text-proph-on-dark"> 訂單簿</strong>，
          以價格優先、時間優先撮合。平台 <strong className="font-semibold text-proph-on-dark">不掛單、不報價、不從輸贏抽成</strong>。
        </p>

        <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-on-dark/15 p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-body-muted">
              平台收的
            </div>
            <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              手續費（雙邊收取）
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
              依交易量計算，與你的勝負無關。我們希望市場活絡，不希望某一邊輸太多。
            </p>
          </article>
          <article className="rounded-proph-lg border border-proph-on-dark/15 p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-body-muted">
              平台不收的
            </div>
            <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              玩家輸贏的差額
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
              SPEC-001 §1.1 第二原則：平台不下場。我們不靠玩家輸錢賺錢；那會把我們變成莊家，是法律紅線。
            </p>
          </article>
        </div>

        <div className="mt-8">
          <ProphLinkButton variant="primary" href="/markets/wbsc-2026-tw-semifinal">
            看真實訂單簿範例
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}

function PricingBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">價格用 cent 表示</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            每一股的價格是 1–99 cent（1 cent = 0.01 點）。可以把價格讀成「市場認為這件事發生的機率」。
            買 YES 50¢ ≈ 市場認為機率 50/50；買 YES 80¢ ≈ 市場認為機率 80%。
          </p>
        </header>

        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
          <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            一個具體例子
          </h3>
          <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            假設「中華隊晉級 WBSC 四強」目前 YES 報價 38¢。你買 100 股 YES：
          </p>

          <dl className="mt-5 grid grid-cols-1 gap-4 md:grid-cols-3">
            <div className="rounded-proph-md border border-proph-hairline bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                成本
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-ink">
                38 點
              </dd>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                100 股 × 38¢ ÷ 100 = 38 點
              </p>
            </div>
            <div className="rounded-proph-md border border-proph-feedback-success/30 bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-success">
                結算為 YES
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-feedback-success">
                +62 點
              </dd>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                每股入帳 100 點，總入帳 100 點 − 38 點成本
              </p>
            </div>
            <div className="rounded-proph-md border border-proph-feedback-error/30 bg-proph-canvas-parchment p-5">
              <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-error">
                結算為 NO
              </dt>
              <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-feedback-error">
                −38 點
              </dd>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                每股入帳 0 點，全額成本損失
              </p>
            </div>
          </dl>

          <p className="mt-5 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
            想中途離場？可以在訂單簿掛賣單，由其他玩家接手。沒有「平台買回」這件事，只有玩家對玩家。
          </p>
        </article>
      </div>
    </section>
  );
}

function SettlementBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">結算來自 Oracle</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            每個市場上架時，platform 已指定明確的判定來源（Oracle）。市場關盤後，依 Oracle 公告判定 YES 或 NO，自動結算進玩家錢包。
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {[
            {
              t: "判定來源透明公開",
              b: "市場頁清楚列出 oracle source（例如：中央氣象署、FOMC 公告、選舉委員會）。沒有「我們覺得」這種判定。",
            },
            {
              t: "24 小時申訴窗口",
              b: "結算後 24 小時內可開申訴工單。所有申訴會收到狀態回覆；多人申訴或大額個案會升級到 Compliance T3。",
            },
            {
              t: "結算狀態機",
              b: "PENDING → SETTLING → SETTLED 或 CANCELLED。每個狀態的轉換寫入事件流，可在個人紀錄頁查到。",
            },
            {
              t: "持倉自動兌換",
              b: "結算當下系統把持倉轉為點數入帳，不需任何手動操作。可在個人持倉頁查看入帳結果。",
            },
          ].map((item) => (
            <li key={item.t} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.t}</h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.b}
              </p>
            </li>
          ))}
        </ul>

        <div className="mt-8">
          <ProphLinkButton variant="secondary-pill" href="/support/disputes">
            申訴流程
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}

function ComplianceBlock() {
  return (
    <section className="bg-proph-surface-tile-2 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          合規邊界
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          不是所有人都能玩，這是設計，不是 bug。
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          18 歲以下不可參與。部分司法管轄區因法律因素無法提供服務。月儲值上限強制設定。
          受過 RG 認證的客服處理沉迷個案。我們把這些邊界視為產品的一部分，不是阻力。
        </p>

        <div className="mt-8 flex flex-wrap items-center gap-4">
          <ProphLinkButton variant="primary" href="/responsible-gaming">責任博弈工具</ProphLinkButton>
          <a
            href="/legal/geo-block"
            className="font-text text-[17px] tracking-[-0.374px] text-proph-primary-on-dark hover:underline"
          >
            服務地區限制 →
          </a>
          <a
            href="/kyc"
            className="font-text text-[17px] tracking-[-0.374px] text-proph-primary-on-dark hover:underline"
          >
            身份驗證分層 →
          </a>
        </div>
      </div>
    </section>
  );
}

export default function HowItWorksPage() {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <Hero />
      <PrinciplesBlock />
      <StartStepsBlock />
      <MatchingBlock />
      <PricingBlock />
      <SettlementBlock />
      <ComplianceBlock />

      <ProphTile
        surface="light"
        kicker="準備好了嗎？"
        headline="從註冊開始，10 分鐘內就能瀏覽你的第一個市場。"
        tagline="基礎驗證 10 秒；責任博弈設定 1 分鐘；儲值入帳 5 分鐘以內。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">立即註冊</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">瀏覽市場</ProphLinkButton>
          </>
        }
      />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
