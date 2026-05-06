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
  title: "結算與爭議 — prophetica",
  description: "Oracle 結算流程、24 小時申訴窗口、Compliance 升級條件與多人申訴處理。",
};


// SPEC-004 §2 settlement state machine.
const states = [
  {
    id: "CLOSED",
    label: "市場關盤",
    body: "達到 ClosesAt 時間，撮合引擎停止接受新訂單；既有訂單照常處理至 in-flight 全數結束。",
  },
  {
    id: "PENDING",
    label: "PENDING — 24 小時申訴窗口",
    body: "Oracle 提交初判結果。系統公告「初判為 YES / NO」並開放 24 小時申訴。期間玩家可開申訴工單質疑判定。",
  },
  {
    id: "SETTLING",
    label: "SETTLING — 結算流水線",
    body: "申訴窗口結束後，系統進入結算流水線：每位持倉者的部位計算為 點 入帳。",
  },
  {
    id: "SETTLED",
    label: "SETTLED — 結算完成",
    body: "玩家錢包已入帳，市場頁顯示最終結果與結算時間。狀態為終態，不再轉換。",
  },
  {
    id: "CANCELLED",
    label: "CANCELLED — 取消結算",
    body: "Oracle 來源失效或申訴成立 → 市場取消。所有持倉以儲值原價退回，玩家不賺不賠。",
  },
];

function HeroBlock() {
  return (
    <ProphTile
      surface="light"
      kicker="結算與爭議"
      headline="公平結算，透明爭議。"
      tagline="從市場關盤到玩家錢包入帳的每一步都記錄在事件流上。我們公開 Oracle 來源、保留 24 小時申訴窗口、把申訴升級條件寫進 SPEC。"
      actions={
        <>
          <ProphLinkButton variant="primary" href="/markets/settled">看已結算市場</ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/support/disputes">申訴流程</ProphLinkButton>
        </>
      }
    />
  );
}

function StateMachineBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">結算狀態機</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            SPEC-004 §2 規定的狀態流。每一次狀態轉換寫入事件流，可在市場頁顯示時間軸。
          </p>
        </header>

        <ol className="space-y-3">
          {states.map((s, i) => (
            <li
              key={s.id}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[12px] tabular-nums tracking-[-0.12px] text-proph-ink-muted-48">
                  {String(i + 1).padStart(2, "0")}
                </span>
                <code className="rounded-proph-sm border border-proph-divider-soft bg-proph-canvas-parchment px-2 py-0.5 font-mono text-[12px] tracking-[-0.12px] text-proph-ink">
                  {s.id}
                </code>
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{s.label}</h3>
              </div>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {s.body}
              </p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function OracleBlock() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">Oracle 來源透明</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            每個市場上架時就在頁面明確列出 oracle source。判定不能依「我們覺得」做出。
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {[
            { t: "單一權威來源", b: "中央氣象署、選舉委員會、聯邦準備銀行、頒獎機構等。對應市場類別已在 SPEC-015 §5.1 列舉。" },
            { t: "雙來源交叉", b: "高金額或高敏感市場走「主來源 + 二手媒體」交叉比對；不一致則進入 PENDING 額外審查。" },
            { t: "金融指標走中位", b: "BTC / 股價類市場採三家獨立交易所中位，避免單一來源被操縱。" },
            { t: "結果不可判定", b: "Oracle 來源失效 / 事件未實際發生 → 進入 CANCELLED，所有持倉以儲值原價退回。" },
          ].map((item) => (
            <li key={item.t} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
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

function DisputeBlock() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">24 小時申訴窗口</h2>
          <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            初判公告後開放 24 小時。所有申訴都會收到狀態回覆（即使尚未裁決）。
          </p>
        </header>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              標準申訴
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              T2 Compliance 客服處理。24 小時內首次回應、48 小時內裁決。裁決結果為最終，可申請進一步審查（升級 T3）。
            </p>
          </article>

          <article className="rounded-proph-lg border border-proph-feedback-warning/40 bg-proph-canvas p-6">
            <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              直接升級 T3 的條件
            </h3>
            <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              <li>· 案件涉及金額 &gt; USD 1,000</li>
              <li>· 多人申訴同一市場</li>
              <li>· T2 無法判定</li>
              <li>· Oracle 來源本身有爭議</li>
            </ul>
          </article>
        </div>

        <div className="mt-8">
          <ProphLinkButton variant="primary" href="/support/disputes">提交申訴</ProphLinkButton>
        </div>
      </div>
    </section>
  );
}

function NeutralityReminder() {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          底線
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          結算的目標是準確，不是讓平台賺錢。
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          因為平台不持倉、不從輸贏抽成，我們沒有動機把任何一筆判定向「讓更多玩家輸」的方向偏。
          錯的判定會傷害品牌、引發監管查核、推高客服成本 — 三條都對 prophetica 有害。
          這不是道德承諾，是利益對齊。
        </p>
      </div>
    </section>
  );
}

export default async function SettlementPage() {
  const sectionFooterColumns = await getFooterColumns(["markets", "legal", "support"]);
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />
      <HeroBlock />
      <StateMachineBlock />
      <OracleBlock />
      <DisputeBlock />
      <NeutralityReminder />
      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
