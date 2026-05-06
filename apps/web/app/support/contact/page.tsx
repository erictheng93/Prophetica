import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "聯絡我們 — prophetica",
  description: "客服管道、SLA 與服務時段。",
};

interface Channel {
  label: string;
  detail: string;
  sla: string;
  hours: string;
  bestFor: string;
}

// SPEC-012 §2 — channel matrix presented to end users.
const channels: Channel[] = [
  {
    label: "站內工單",
    detail: "登入後在帳號頁建立工單；可附截圖 / PDF。",
    sla: "24 小時內首次回應",
    hours: "全時段（系統收件）",
    bestFor: "帳號 / 結算 / 一般爭議",
  },
  {
    label: "Email",
    detail: "service@（執照地確認後公告完整 email）",
    sla: "48 小時內首次回應",
    hours: "全時段（系統收件）",
    bestFor: "可附較長文件、不急的詢問",
  },
  {
    label: "Live Chat",
    detail: "右下角浮動視窗，須登入。中文 / 英文 / 日文。",
    sla: "5 分鐘內回應",
    hours: "24 / 7",
    bestFor: "已登入用戶緊急狀況",
  },
  {
    label: "電話客服",
    detail: "限 Compliance 與紛爭升級個案；不接受一般查詢。",
    sla: "4 小時內回撥",
    hours: "週一至週五 09:00–18:00（執照地時區）",
    bestFor: "RG 高風險、Compliance 升級",
  },
];

function ChannelCards() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">服務管道</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            依問題性質選最合適的管道。緊急狀況一律走 Live Chat，受過認證的客服優先接案。
          </p>
        </header>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {channels.map((c) => (
            <li
              key={c.label}
              className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {c.bestFor}
              </div>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {c.label}
              </h3>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.detail}
              </p>
              <dl className="mt-auto grid grid-cols-2 gap-4 border-t border-proph-divider-soft pt-4">
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    SLA
                  </dt>
                  <dd className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-primary">{c.sla}</dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    時段
                  </dt>
                  <dd className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">{c.hours}</dd>
                </div>
              </dl>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function GeneralForm() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">一般詢問</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            非帳號相關詢問可在這裡留言。需要查帳號或交易紀錄請直接登入後開工單，會更快。
          </p>
        </header>

        <form action="#" className="space-y-6">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                你的稱呼
              </span>
              <input
                type="text"
                name="name"
                placeholder="王小明"
                className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                聯絡 Email
              </span>
              <input
                type="email"
                name="email"
                placeholder="you@example.com"
                className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
          </div>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              主旨
            </span>
            <input
              type="text"
              name="subject"
              placeholder="一句話描述你想問的事"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              訊息內容
            </span>
            <textarea
              name="body"
              rows={6}
              placeholder="請說明你的問題與你已嘗試的步驟。"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <p className="font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
            提交即代表你同意我們依{" "}
            <a href="/legal/privacy" className="text-proph-primary underline">
              隱私政策
            </a>{" "}
            處理你提供的聯絡資料。我們不會把你的 Email 用於行銷推播。
          </p>

          <div className="flex justify-end">
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              送出（後端待接）
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default function ContactPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="客服 / 聯絡我們"
        headline="找最合適的管道，最快得到回應。"
        tagline="不同問題走不同管道。已登入用戶建議走站內工單與 Live Chat；未登入或一般詢問走下方表單與 Email。"
      />
      <ChannelCards />
      <GeneralForm />
    </>
  );
}
