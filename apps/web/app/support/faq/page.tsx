import type { Metadata } from "next";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "常見問題 — prophetica",
  description: "帳號、儲值、KYC、結算、責任博弈、安全等常見問題與解答。",
};

interface FAQ {
  q: string;
  a: React.ReactNode;
}

interface Section {
  id: string;
  title: string;
  intro?: string;
  items: FAQ[];
}

const sections: Section[] = [
  {
    id: "account",
    title: "帳號",
    items: [
      {
        q: "我忘記密碼怎麼辦？",
        a: "在登入頁點「忘記密碼」，系統會將重設連結寄至註冊 Email。連結 24 小時內有效。",
      },
      {
        q: "收不到驗證碼？",
        a: "檢查垃圾郵件夾，確認 Email 拼寫無誤；仍未收到請開工單。",
      },
      {
        q: "為什麼帳戶被凍結？",
        a: "凍結原因會以 Email 與站內訊息通知。常見原因：KYC 重啟、自我排除生效、可疑活動偵測。可透過工單申請審核。",
      },
      {
        q: "可以同時擁有多個帳號嗎？",
        a: "不可。SPEC-001 ToS-007 規定洗售 / 多帳號 / 操縱行為將永久停權。",
      },
    ],
  },
  {
    id: "topup",
    title: "儲值",
    items: [
      {
        q: "儲值後點數沒入帳？",
        a: "通常 5 分鐘內入帳。超過 30 分鐘請附訂單編號開工單，金流組會優先處理。",
      },
      {
        q: "點數可以兌回現金嗎？",
        a: (
          <>
            <strong className="font-semibold">不可。</strong> 站內點數僅限站內使用，不可兌回現金、虛擬貨幣或實體商品。這是
            prophetica 的法律邊界（SPEC-001 §1.1 三原則之一）。
          </>
        ),
      },
      {
        q: "為什麼有月儲值上限？",
        a: "責任博弈機制（SPEC-006 §3.1）的強制保護。Tier 1 為 TWD 30,000；升級 Tier 2 / 3 可解鎖更高上限。",
      },
      {
        q: "降低儲值上限後可以馬上提高嗎？",
        a: "降低立即生效；提高需 24 小時冷卻期，且過去 30 日無風險訊號才會核准。",
      },
    ],
  },
  {
    id: "kyc",
    title: "身份驗證 KYC",
    items: [
      {
        q: "為什麼要做 KYC？",
        a: "合規與反洗錢要求。我們採分層 KYC，Tier 1 最低層級只需姓名 / 生日 / 國家 / 證件號（不需上傳影像，10 秒完成）。",
      },
      {
        q: "KYC 失敗了怎麼辦？",
        a: "24 小時內最多 5 次嘗試。多次失敗會自動引導至 Tier 2 SumSub 完整流程；仍無法通過請聯繫客服。",
      },
      {
        q: "我的證件影像如何處理？",
        a: (
          <>
            由 SumSub Cloud（EU region）與 Cloudflare R2（EU region）Object Lock 加密儲存。
            <strong className="font-semibold">客服無法直接讀取證件影像</strong>，只有 Compliance Officer 在通過簽核後可申請查閱。
            完整說明見 <a href="/kyc/privacy-notice" className="text-proph-primary hover:underline">KYC 隱私說明</a>。
          </>
        ),
      },
    ],
  },
  {
    id: "settlement",
    title: "結算與市場",
    items: [
      {
        q: "結算結果如何判定？",
        a: "由 SPEC-004 規定的 Oracle 來源判定。每個市場頁面會明確列出 oracle source（例如：中央氣象署官網、Federal Reserve 官方公告）。",
      },
      {
        q: "對結算結果有疑問？",
        a: (
          <>
            24 小時申訴窗口開放，可在{" "}
            <a href="/support/disputes" className="text-proph-primary hover:underline">
              申訴流程
            </a>
            提交。所有申訴 24 小時內會收到狀態回覆（即使尚未裁決）。
          </>
        ),
      },
      {
        q: "市場可以提前下架嗎？",
        a: "可以。SPEC-015 §4 賦予 Compliance Officer 緊急下架權，任何狀態皆可強制下架（記錄理由）。下架後既有持倉照常結算。",
      },
    ],
  },
  {
    id: "rg",
    title: "責任博弈",
    items: [
      {
        q: "我覺得自己玩太多了，怎麼辦？",
        a: (
          <>
            馬上前往{" "}
            <a href="/responsible-gaming" className="text-proph-primary hover:underline">
              責任博弈
            </a>{" "}
            設定限額或啟動冷靜期。外部援助專線{" "}
            <a href="tel:1925" className="text-proph-primary hover:underline">
              1925
            </a>
            （衛福部安心專線）24 小時免費，且不會與 prophetica 交換任何資訊。
          </>
        ),
      },
      {
        q: "自我排除可以解除嗎？",
        a: "Time-out（6 個月 / 1 年）到期後可申請解除，並有 14 天延長冷靜期。永久排除無法解除。",
      },
      {
        q: "親友可以代為申請停權嗎？",
        a: "我們接受第三方通報，但不會直接以親友通報為據停權；通報觸發系統 review，可能聯繫玩家邀請其自我排除（SPEC-006 §5.4）。",
      },
    ],
  },
  {
    id: "security",
    title: "安全",
    items: [
      {
        q: "帳戶被盜怎麼辦？",
        a: "立即聯繫客服（緊急 — 4 小時優先回應）。我們會凍結帳戶並啟動 Security 團隊審查。建議完成 KYC 後立即啟動兩步驗證。",
      },
      {
        q: "我可以開啟兩步驗證嗎？",
        a: "可以。建議使用 TOTP（Google Authenticator / 1Password / Authy）。SMS OTP 為 fallback；高活動量帳戶會被要求升級 step-up MFA。",
      },
    ],
  },
];

function SectionBlock({ section }: { section: Section }) {
  return (
    <section id={section.id} className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-6">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">{section.title}</h2>
          {section.intro ? (
            <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">{section.intro}</p>
          ) : null}
        </header>
        <ul className="space-y-3">
          {section.items.map((item, i) => (
            <li
              key={`${section.id}-${i}`}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <details className="group">
                <summary className="flex cursor-pointer list-none items-baseline justify-between gap-4">
                  <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
                    {item.q}
                  </h3>
                  <span className="shrink-0 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48 transition-transform group-open:rotate-180">
                    ▾
                  </span>
                </summary>
                <div className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {item.a}
                </div>
              </details>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

export default function FAQPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="客服 / 常見問題"
        headline="先看這裡，多半能解決。"
        tagline="我們收集了過去半年最常被問的問題。沒找到答案？開工單或前往聯絡頁。"
      />

      {/* In-page anchor TOC */}
      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph overflow-x-auto px-6 py-6 md:px-10">
          <ul className="flex items-center gap-2">
            {sections.map((s) => (
              <li key={s.id}>
                <a
                  href={`#${s.id}`}
                  className="inline-flex items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-4 py-1.5 font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink"
                >
                  {s.title}
                </a>
              </li>
            ))}
          </ul>
        </div>
      </section>

      {sections.map((s) => (
        <SectionBlock key={s.id} section={s} />
      ))}

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 text-center md:px-10">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">沒找到答案？</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            開工單，平均 24 小時內首次回應。緊急安全事件 4 小時內優先處理。
          </p>
          <div className="mt-6 flex flex-wrap items-center justify-center gap-4">
            <ProphLinkButton variant="primary" href="/support/contact">聯絡我們</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/support/disputes">申訴流程</ProphLinkButton>
          </div>
        </div>
      </section>
    </>
  );
}
