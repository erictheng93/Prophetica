import type { Metadata } from "next";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "客服中心 — prophetica",
  description: "24/7 客服體系。一般查詢、儲值、KYC、結算、責任博弈、安全事件。",
};

interface ChannelCard {
  label: string;
  sla: string;
  desc: string;
  hours: string;
}

// SPEC-012 §2 — channel matrix.
const channels: ChannelCard[] = [
  { label: "站內工單", sla: "24 小時內首次回應", desc: "適用所有帳號 / 結算 / 一般爭議。中文 / 英文。", hours: "全時段" },
  { label: "Email", sla: "48 小時內首次回應", desc: "service@（執照地確認後公告）。可附文件。", hours: "全時段" },
  { label: "Live Chat", sla: "5 分鐘內回應", desc: "限已登入用戶。中文 / 英文 / 日文。", hours: "24/7" },
  { label: "電話客服", sla: "4 小時內回撥", desc: "限 RG 高風險、Compliance、紛爭升級個案。", hours: "辦公時間" },
];

const categories = [
  { kicker: "帳號", title: "登入 / 註冊 / 兩步驗證", body: "找不到驗證碼、密碼重設、帳戶被鎖定。", route: "/support/faq#account" },
  { kicker: "儲值", title: "金流問題", body: "點數未入帳、支付失敗、月儲值上限。", route: "/support/faq#topup" },
  { kicker: "KYC", title: "身份驗證", body: "Tier 升級失敗、SumSub 流程、證件保存疑問。", route: "/support/faq#kyc" },
  { kicker: "結算", title: "Oracle 與爭議", body: "對結算結果有疑問、24 小時申訴。", route: "/support/disputes" },
  { kicker: "安全", title: "帳戶被盜", body: "立即聯繫客服，4 小時內優先回應。", route: "/support/contact" },
  { kicker: "責任博弈", title: "我玩太多了", body: "限額、冷靜期、自我排除、外部援助。", route: "/responsible-gaming" },
];

function ChannelGrid() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">服務管道</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            依問題性質選擇最快的管道。RG 與 Compliance 個案由受過認證的客服處理。
          </p>
        </header>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          {channels.map((c) => (
            <article key={c.label} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {c.hours}
              </div>
              <h3 className="mt-1 font-display text-[21px] font-semibold tracking-[0.231px]">{c.label}</h3>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-primary">{c.sla}</div>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.desc}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function CategoryGrid() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">依主題分流</h2>
        </header>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
          {categories.map((c) => (
            <a
              key={c.title}
              href={c.route}
              className="group rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {c.kicker}
              </span>
              <h3 className="mt-1 font-display text-[21px] font-semibold tracking-[0.231px] text-proph-ink group-hover:underline">
                {c.title}
              </h3>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.body}
              </p>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}

function RGCallout() {
  // SPEC-012 §6.2 — Help Center must surface a prominent RG help entry.
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          責任博弈優先
        </span>
        <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
          如果你或身邊的人覺得遊戲已失去掌控，這條入口優先於所有其他協助。
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          我們有受過認證的責任博弈客服、限額 / 冷靜期 / 自我排除工具，
          以及獨立於 prophetica 之外的外部專業資源。撥打援助專線完全免費，且不會與 prophetica 交換任何資訊。
        </p>
        <div className="mt-8 flex flex-wrap items-center gap-4">
          <ProphLinkButton variant="primary" href="/responsible-gaming">
            了解責任博弈工具
          </ProphLinkButton>
          <a
            href="tel:1925"
            className="font-mono text-[28px] font-semibold tabular-nums text-proph-primary-on-dark hover:underline"
          >
            1925
            <span className="ml-2 font-text text-[14px] tracking-[-0.224px] text-proph-body-muted">
              衛福部安心專線 · 24 小時
            </span>
          </a>
        </div>
      </div>
    </section>
  );
}

export default function SupportHubPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="客服中心"
        headline="找對人，問對問題。"
        tagline="我們把所有客服管道與常見議題整理在這裡。複雜個案會由受過合規培訓的 T2 / T3 客服接手；責任博弈與 Compliance 案件優先處理。"
      />
      <ChannelGrid />
      <CategoryGrid />
      <RGCallout />
    </>
  );
}
