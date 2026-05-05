import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "冷靜期 — 責任博弈 — prophetica",
  description: "暫停 24 小時、7 天或 30 天。期間無法登入、儲值與交易。",
};

// SPEC-006 §5.1 — Cool-off has 3 fixed durations.
const durations = [
  { value: "24h", label: "24 小時", body: "短暫休息。最常用於想中斷一次衝動性的下單。" },
  { value: "7d", label: "7 天", body: "中度暫停。適合工作 / 學業壓力期或情緒波動時使用。" },
  { value: "30d", label: "30 天", body: "深度暫停。讓自己徹底離開一個月，重新評估遊戲節奏。" },
];

function FrozenWhatHappens() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          冷靜期內會發生什麼
        </h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {[
            { title: "無法登入", body: "瀏覽器、行動裝置一律被拒。客服也無法協助提前解除。" },
            { title: "無法儲值與交易", body: "支付通路與訂單 API 對你的帳號關閉。" },
            { title: "持倉照常結算", body: "已開倉的對局照原本規則結算，結果存入凍結錢包，到期解凍。" },
            { title: "通知靜音", body: "Email、推播、客服訊息全面停止，避免拉回欲望。" },
          ].map((item) => (
            <li key={item.title} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{item.title}</h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {item.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function ChooseDuration() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">選擇冷靜期長度</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            到期自動解除，無需任何操作。期限內無法縮短或取消。
          </p>
        </header>

        <form action="#" aria-disabled="true" className="space-y-6">
          <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <legend className="sr-only">冷靜期長度選項</legend>
            {durations.map((d) => (
              <label
                key={d.value}
                className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas p-5 transition-colors has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
              >
                <span className="flex items-center gap-3">
                  <input type="radio" name="duration" value={d.value} className="h-4 w-4 accent-proph-primary" />
                  <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                    {d.label}
                  </span>
                </span>
                <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {d.body}
                </span>
              </label>
            ))}
          </fieldset>

          <div className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas-parchment px-5 py-4">
            <p className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
              冷靜期一旦啟動即無法取消。若需要更長期或永久停止使用，請使用
              <a href="/responsible-gaming/self-exclusion" className="underline">
                自我排除
              </a>
              。
            </p>
          </div>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <label className="flex items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              <input type="checkbox" required className="h-4 w-4 accent-proph-primary" />
              我了解冷靜期啟動後無法縮短或取消。
            </label>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              啟動冷靜期（後端待接）
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default function CooldownPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="責任博弈 / 冷靜期"
        headline="休息一下，比加碼更重要。"
        tagline="冷靜期是給你自己的暫停鍵。期間 prophetica 完全把門關上 ── 帳戶凍結、通知靜音 ── 直到你準備好再開始。"
      />
      <FrozenWhatHappens />
      <ChooseDuration />
    </>
  );
}
