import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "Tier 1 基礎驗證 — KYC — prophetica",
  description: "首發 5 國對應證件的隱式驗證。10 秒完成，無需上傳證件影像。",
  robots: { index: false, follow: false },
};

// SPEC-005 §2.4 — first-launch 5 countries with verification-method differences.
// PH (PhilSys / passport) is intentionally weaker on Tier 1; failures route to Tier 2.
interface CountrySpec {
  code: string;
  flag: string;
  name: string;
  document: string;
  format: string;
  notes: string;
}

const countries: CountrySpec[] = [
  {
    code: "TW",
    flag: "🇹🇼",
    name: "台灣",
    document: "身份證號",
    format: "1 字母 + 9 碼數字（10 位）",
    notes: "Checksum 驗證。我們只儲存雜湊（SHA256），不存原始號碼。",
  },
  {
    code: "JP",
    flag: "🇯🇵",
    name: "日本",
    document: "個人番号（マイナンバー）",
    format: "12 碼數字",
    notes: "不收マイナンバーカード影像。額外要求姓名（ローマ字）+ 生日 + 居住都道府縣。",
  },
  {
    code: "KR",
    flag: "🇰🇷",
    name: "韓國",
    document: "주민등록번호",
    format: "13 碼數字（YYMMDD-XXXXXXX）",
    notes: "依韓國法規，本欄位完全不入庫。我們只生成雜湊做唯一性比對，提交後立即丟棄原值。",
  },
  {
    code: "VN",
    flag: "🇻🇳",
    name: "越南",
    document: "公民身份證 CCCD",
    format: "12 碼（舊版 CMND 9 碼亦支援）",
    notes: "舊版紙本式樣多元；若 OCR 不過將自動引導至 Tier 2 流程。",
  },
  {
    code: "MY",
    flag: "🇲🇾",
    name: "馬來西亞",
    document: "NRIC / MyKad",
    format: "12 碼（YYMMDD-PB-XXXG）",
    notes: "中段 6 碼為 YYMMDD，內含年齡可直接驗證 ≥ 18。",
  },
];

function PrivacyReminder() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-6">
          <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
            送出前請先確認
          </h2>
          <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            填寫此表前請先閱讀
            <a href="/kyc/privacy-notice" className="text-proph-primary hover:underline">
              KYC 隱私說明
            </a>
            ，了解 prophetica 收集哪些欄位、為什麼收集、保存多久、由誰存取。Tier 1 的證件號碼一律
            <strong className="font-semibold text-proph-ink">只存雜湊</strong>，原始值不入庫。
          </p>
        </article>
      </div>
    </section>
  );
}

function CountryGuide() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">首發 5 國的證件規格</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            選擇你的國家後，在下方表單依對應格式輸入。不在這 5 國的玩家，請先閱讀
            <a href="/legal/geo-block" className="text-proph-primary hover:underline">
              服務地區限制
            </a>
            。
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {countries.map((c) => (
            <li
              key={c.code}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline gap-3">
                <span aria-hidden className="text-[24px]">
                  {c.flag}
                </span>
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">{c.name}</h3>
              </div>
              <div className="mt-3 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                證件
              </div>
              <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink">{c.document}</p>
              <div className="mt-2 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                格式
              </div>
              <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.format}
              </p>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {c.notes}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function Tier1Form() {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">提交基礎驗證</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            驗證在後端完成，**不**需要上傳證件影像。送出後即時通過或拒絕。
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <label className="flex flex-col gap-2">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                姓名（與證件一致）
              </span>
              <input
                type="text"
                name="legalName"
                placeholder="王小明"
                className="rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>

            <label className="flex flex-col gap-2">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                生日
              </span>
              <input
                type="date"
                name="birthday"
                className="rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
              />
            </label>

            <label className="flex flex-col gap-2">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                國家
              </span>
              <select
                name="country"
                className="rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
              >
                <option value="">— 請選擇 —</option>
                {countries.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.name}（{c.document}）
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-2 md:col-span-1">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                證件號碼
              </span>
              <input
                type="text"
                name="documentId"
                inputMode="text"
                autoComplete="off"
                spellCheck={false}
                placeholder="請依上方對應格式輸入"
                className="rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
          </div>

          <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
            <span>
              我已閱讀
              <a href="/kyc/privacy-notice" className="text-proph-primary hover:underline">
                KYC 隱私說明
              </a>
              與
              <a href="/legal/terms" className="text-proph-primary hover:underline">
                服務條款
              </a>
              ，並同意 prophetica 依據說明處理我提交的資料。
            </span>
          </label>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              送出後 24 小時內最多嘗試 5 次。連續失敗將導入 Tier 2 流程。
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              送出基礎驗證（後端待接）
            </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

export default function Tier1Page() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="KYC / Tier 1 基礎"
        headline="10 秒完成最低必要驗證。"
        tagline="姓名、生日、國家與證件號碼。我們在後端用對應國的 checksum / format 規則驗證真偽，不上傳任何證件影像。"
      />
      <PrivacyReminder />
      <CountryGuide />
      <Tier1Form />
    </>
  );
}
