import type { Metadata } from "next";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import { Brand } from "@/lib/site-nav";

// SPEC-001 §2 — Geo-block user-facing screen.
// Linked from footer (/legal/geo-block) AND served as the interception page by
// middleware (apps/web/middleware.ts) when Layer 1/2 IP geolocation flags the
// request as originating from a 🔴 jurisdiction.
//
// Compliance posture (SPEC-001 §1.1 + §2.2):
//   - DO NOT list specific country names — first-launch 🔴 list is pending
//     final legal opinion (SPEC-000 §1 note). Naming countries pre-opinion
//     creates a documentary gap.
//   - DO NOT suggest VPN, proxy, IP rotation, or any circumvention path.
//   - Support contact is for misclassification appeals only — explicit framing
//     so it cannot be read as a workaround channel.
//   - This page must be reachable without authentication and without geo-check
//     (it IS the destination of geo-block redirects).

export const metadata: Metadata = {
  title: "服務地區限制 — prophetica",
  description:
    "依法律規範與牌照範圍，本服務無法在部分地區提供。如有疑問請透過客服管道聯繫。",
  robots: { index: false, follow: false },
};

const footerColumns = [
  {
    heading: "法律與合規",
    links: [
      { label: "服務條款", href: "/legal/terms" },
      { label: "隱私政策", href: "/legal/privacy" },
      { label: "AML 政策", href: "/legal/aml" },
    ],
  },
  {
    heading: "支援",
    links: [
      { label: "客服中心", href: "/support" },
      { label: "申訴流程", href: "/support/disputes" },
    ],
  },
];

export default function GeoBlockPage() {
  return (
    <main className="min-h-screen bg-proph-canvas">
      {/*
       * Geo-block screen runs a deliberately reduced primary nav: only the
       * brand and a support link. Auth / market entry CTAs are intentionally
       * omitted so this page never reads as "find a way around the block."
       * SPEC-001 §2.2 forbids any UI hint suggesting circumvention.
       */}
      <ProphSubNav
        category={<Brand />}
        cta={
          <a
            href="/support"
            className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
          >
            聯繫客服
          </a>
        }
      />

      <ProphTile
        surface="light"
        kicker="服務地區限制"
        headline={
          <span className="font-display text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            本服務在你目前的所在地區無法使用。
          </span>
        }
        tagline={
          <>
            依當地法律規範與本平台牌照範圍，prophetica 無法在所有司法管轄區提供服務。
            <br />
            這不是技術問題，而是合規範圍的明確劃定。
          </>
        }
      />

      <ProphTile
        surface="parchment"
        headline="關於這項判定"
        tagline={
          <>
            判定依 IP 地理資訊與註冊資料雙重比對。我們不會記錄 IP 以外的位置資料，
            也不會儲存無法被合規追溯的識別資訊。
          </>
        }
      />

      {/*
       * Misclassification appeal channel — framed strictly as "we got it wrong",
       * never as "find a way around this." SPEC-001 §2.2 forbids any UI hint
       * that could be read as circumvention guidance.
       */}
      <ProphTile
        surface="light"
        headline="認為是誤判？"
        tagline={
          <>
            如果你**長期居住於可服務地區**，僅因短期出差或網路路由導致誤判，
            可透過客服管道提交申訴。請準備居住地證明文件以利人工審核。
          </>
        }
        actions={<ProphLinkButton variant="primary" href="/support">聯繫客服</ProphLinkButton>}
      />

      <ProphFooter
        columns={footerColumns}
        legal={
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <span>
              © {new Date().getFullYear()} prophetica. 趣味預測娛樂遊戲服務 · 18+ ·
              站內點數不可兌回現金 / 虛擬貨幣 / 實體商品。
            </span>
            <span className="font-text text-[12px] leading-none tracking-[-0.12px]">
              援助專線 <a href="tel:1925" className="text-proph-primary hover:underline">1925</a>（24 小時）
            </span>
          </div>
        }
      />
    </main>
  );
}
