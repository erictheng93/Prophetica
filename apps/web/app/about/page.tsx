import type { Metadata } from "next";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";

export const metadata: Metadata = {
  title: "關於 prophetica — 中立的趣味預測娛樂平台",
  description: "prophetica 不持倉、不下場，只做撮合與結算。三條法律邊界、三公司架構、為什麼是娛樂遊戲不是博弈。",
};


const principles = [
  { kicker: "原則一", title: "點數不可兌回", body: "永遠不可兌回現金、虛擬貨幣或實體商品。" },
  { kicker: "原則二", title: "平台不下場", body: "不持倉、不對賭、不從輸贏抽成。" },
  { kicker: "原則三", title: "全程稽核留痕", body: "金流 / 撮合 / 結算 / 判定皆寫入不可竄改事件流，保存 7 年以上。" },
];

const corporateStructure = [
  {
    name: "Holding Co.",
    location: "BVI",
    role: "控股實體",
    body: "持有 Op Co. 與 Tech Co. 股權；不直接面對玩家或執照地監管。",
  },
  {
    name: "Op Co.",
    location: "執照地（Malta / Curaçao / Isle of Man 評估中）",
    role: "營運主體",
    body: "與玩家簽約、收受儲值、處理 KYC 與結算。受執照地金融 / 博弈監管。",
  },
  {
    name: "Tech Co.",
    location: "技術中心（待定）",
    role: "技術開發",
    body: "撮合引擎、ledger、SDK 等核心系統；不持有玩家資金，不收儲值。",
  },
];

function HeroBlock() {
  return (
    <ProphTile
      surface="light"
      kicker="關於 prophetica"
      headline={
        <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
          我們做撮合與結算，平台不下場。
        </span>
      }
      tagline="prophetica 是趣味預測娛樂遊戲。我們把玩家的「我認為會發生」變成可以撮合的訂單，做為對局的中立中間人。手續費是我們唯一的收入。"
      actions={
        <>
          <ProphLinkButton variant="primary" href="/how-it-works">看玩法說明</ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/markets">瀏覽市場</ProphLinkButton>
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
            這些不是我們宣稱的「企業價值」。它們是 prophetica 存在的前提：違反任一條等同關站，創辦人負刑責。
            我們把這三條寫進系統設計、客服培訓、合規稽核，每一行 production code 都受其約束。
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

function NeutralityBlock() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          中立的承諾
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          手續費不靠玩家輸贏。
        </h2>
        <p className="mt-4 max-w-3xl font-display text-[24px] font-light leading-[1.5] text-proph-body-muted">
          我們依交易量收手續費，不論玩家輸或贏。這個結構讓 prophetica 有動機讓
          <strong className="font-semibold text-proph-on-dark"> 市場活絡</strong>，而不是
          <strong className="font-semibold text-proph-on-dark"> 玩家輸錢</strong>。
        </p>

        <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-3">
          {[
            {
              t: "Oracle 來源透明",
              b: "每個市場上架時就指定明確判定來源。沒有「平台覺得」這種判定。",
            },
            {
              t: "禁止平台帳戶下單",
              b: "後端強制白名單；任何 PLATFORM_SYSTEM_USER_IDS 不能成為訂單簿掛單者。",
            },
            {
              t: "結算可追溯",
              b: "從 Oracle 提交到玩家錢包入帳，每一步寫入事件流，可被監管稽核。",
            },
          ].map((item) => (
            <article key={item.t} className="rounded-proph-lg border border-proph-on-dark/15 p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.t}</h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
                {item.b}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function StructureBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">三公司架構</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            把監管、營運、技術三件事放在不同實體，是業界最佳實踐。它讓營運責任、玩家資金、技術開發各有所屬。
            細節可參考{" "}
            <a
              href="https://github.com/cloudflare/kumo"
              className="text-proph-primary underline"
              rel="noreferrer"
            >
              ADR-0002
            </a>{" "}
            三公司架構文件。
          </p>
        </header>
        <ul className="space-y-4">
          {corporateStructure.map((co) => (
            <li
              key={co.name}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex flex-wrap items-baseline justify-between gap-3">
                <div className="flex items-baseline gap-3">
                  <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                    {co.name}
                  </h3>
                  <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {co.role}
                  </span>
                </div>
                <span className="rounded-proph-pill border border-proph-hairline bg-proph-canvas-parchment px-3 py-0.5 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-80">
                  {co.location}
                </span>
              </div>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {co.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function NotGamblingBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
          為什麼說「娛樂遊戲」不是「博弈」？
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          差別不在玩家體驗，而在法律結構。下面三項共同構成「娛樂遊戲」的合規身份；缺一條就會被認定為博弈服務。
        </p>

        <ul className="mt-8 space-y-3">
          {[
            { t: "點數無現金價值", b: "無回兌通道。儲值是「購買遊戲服務」，不是「下注」。" },
            { t: "平台不持倉", b: "平台不參與輸贏。玩家對玩家，不是玩家對莊家。" },
            { t: "完整可稽核", b: "監管可隨時調閱完整資金流與決策軌跡，無黑箱。" },
          ].map((item) => (
            <li key={item.t} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.t}</h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.b}
              </p>
            </li>
          ))}
        </ul>

        <p className="mt-6 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
          這三點同時成立才能避開博弈法律。任何「未來會做的功能」如果違反其一，prophetica 不會做。
        </p>
      </div>
    </section>
  );
}

function ContactBlock() {
  return (
    <ProphTile
      surface="light"
      kicker="聯繫"
      headline="想合作、加入或舉報？"
      tagline="一般客服走支援頁；商務 / 媒體 / 加入請走聯絡管道。Compliance 緊急事件 4 小時內回應。"
      actions={
        <>
          <ProphLinkButton variant="primary" href="/support/contact">聯絡管道</ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/affiliate">代理計畫</ProphLinkButton>
        </>
      }
    />
  );
}

export default async function AboutPage() {
  const sectionFooterColumns = await getFooterColumns(["markets", "responsibleGaming", "legal", "support"]);
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />
      <HeroBlock />
      <PrinciplesBlock />
      <NeutralityBlock />
      <StructureBlock />
      <NotGamblingBlock />
      <ContactBlock />
      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
