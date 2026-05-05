import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "求助資源 — 責任博弈 — prophetica",
  description: "獨立於 prophetica 之外的專業協助、心理衛生與戒癮資源。",
  robots: { index: true, follow: true },
};

// Note: the helpline strip rendered by app/responsible-gaming/layout.tsx
// already lists the primary 24h numbers (1925, 0800-770-885, 1980). This
// page expands with situational guidance and self-assessment pointers.
// Final international resource set is pending Compliance Officer sign-off
// (mirrors SPEC-006 §11 OQ #2 cross-platform exclusion question).

const guides = [
  {
    title: "自我評估",
    body: "先回答幾題簡短問題，了解自己目前的遊戲行為是否需要外部協助。問卷由獨立心理衛生組織設計，prophetica 不會接收答案。",
    cta: "前往自我評估",
    href: "#self-assessment",
  },
  {
    title: "親友求助",
    body: "如果你擔心身邊的人出現沉迷徵兆，請參考親友支援指引。我們不接受第三方代為停權，但你可以引導對方主動聯繫客服。",
    cta: "親友支援指引",
    href: "#family",
  },
  {
    title: "客服 RG 介入",
    body: "若你已使用過冷靜期或自我排除，仍感到難以控制行為，可申請與受過 RG 訓練的客服 1 對 1 評估，協助規劃個人化計畫。",
    cta: "預約客服評估",
    href: "/support/contact",
  },
];

function GuideCards() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">進一步資源</h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {guides.map((g) => (
            <article key={g.title} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{g.title}</h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {g.body}
              </p>
              <a
                href={g.href}
                className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary hover:underline"
              >
                {g.cta} →
              </a>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function PrivacyNote() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="font-display text-[40px] font-semibold leading-[1.1]">關於你的隱私</h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          撥打外部援助專線完全免費，且
          <strong className="font-semibold text-proph-ink">不會與 prophetica 交換任何資訊</strong>。
          我們不會知道你聯絡了哪些機構、討論了什麼。
        </p>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          如果你選擇透過站內客服求助，溝通紀錄將依
          <a href="/legal/privacy" className="text-proph-primary hover:underline">
            隱私政策
          </a>
          處理，僅授權給通過 RG 培訓的客服人員存取。
        </p>
      </div>
    </section>
  );
}

export default function ResourcesPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="責任博弈 / 求助資源"
        headline="你不需要一個人面對。"
        tagline="這個頁面整理的所有資源都獨立於 prophetica 之外。我們把連結放在這裡，但聯繫的是專業機構，不是我們。"
      />
      <GuideCards />
      <PrivacyNote />
    </>
  );
}
