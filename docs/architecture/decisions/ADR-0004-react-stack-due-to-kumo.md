# ADR-0004 — 採用 Cloudflare Kumo 作為元件庫，前端框架由 Nuxt 3 / Vue 3 切換至 Next.js 15 / React 19

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted |
| **Date** | 2026-05-05 |
| **Deciders** | Founder, Tech Lead |
| **Consulted** | 預定：前端工程師（待招募）、設計師（landing page 設計檔審視中）、Codex 開發代理 |
| **Informed** | 全工程團隊、Compliance Officer（無障礙合規影響）、QA |
| **Tags** | frontend · component-library · framework · accessibility · core-architecture |
| **依賴 / 影響** | SPEC-014 §3.2 / §4 / §5 / §8.2 / §9（已同步至 v0.1.2）、PRD（原指定 Nuxt 3）、`design/COMPONENTS.md`、`design/landing-*.html` |

---

## 1. Context and Problem Statement

PRD v0 與 SPEC-014 v0.1.1 原指定前端棧為 **Nuxt 3 (Vue 3) + Pinia + TailwindCSS + ShadCN-Vue 風格化**。此選擇在無外部約束下可行，但 2026-05-05 業主決定**指定 Cloudflare Kumo（`@cloudflare/kumo`）作為本專案元件庫**，理由如下：

1. **無障礙開箱即用**：Kumo 建立於 Base UI（headless React primitives），鍵盤導航、focus management、ARIA 屬性全自動處理，符合 前端合規流程的可及性要求
2. **CSS variables 暴露樣式**：可把 design system token（見 `design/DESIGN-{apple,airbnb,lamborghini}.md` 三候選方案）平移到主題層，免於 fork 元件
3. **官方背書 + MIT license**：Cloudflare 維護、商用零障礙；本專案前端部署本就在 Cloudflare Pages，生態一致
4. **Re-export Base UI primitives**：複雜場景（自訂 dropdown、撮合表單）可降到 Base UI，避免引入第二套 UI 庫

問題是：**Kumo 為 React-only 元件庫，與 Nuxt 3 / Vue 3 不相容**。

驗證資料（2026-05-05 直接讀取 `github.com/cloudflare/kumo`）：

- README 範例：`import { Button, Input, Dialog } from "@cloudflare/kumo"`
- peerDependencies：`react`、`react-dom`、`@phosphor-icons/react`
- repo 內**零 Vue / Nuxt / Svelte / Web Component** 依賴或範例
- 官方未承諾、亦無公開 roadmap 提供 Vue port

故必須在以下兩條路徑中擇一：

- **路徑 X**：維持 Nuxt 3 + Vue 3，放棄 Kumo，另選 Vue 系元件庫（PrimeVue / Vuetify / Naive UI / shadcn-vue）
- **路徑 Y**：採納 Kumo，把前端框架同步切換至 React 系（Next.js / Remix）

本 ADR 即決定走路徑 Y，並選定具體框架。

## 2. Decision Drivers

依重要性遞減：

1. **業主指定的硬約束**：Kumo 為元件庫已是業主決策，本 ADR 範圍是「在此約束下的最佳框架選擇」，不重新討論元件庫選型
2. **無障礙合規**：前端合規流程要求鍵盤導航與螢幕閱讀器支援
3. **SSR / SEO**：landing page、市場詳情頁面（`/markets/[id]`）需被搜尋引擎索引；首屏載入時間影響 SPEC-014 前端 SLO
4. **WebSocket 即時性**：撮合報價需透過 WS 推播，框架必須能在 client component 內優雅整合
5. **Cloudflare Pages 部署親和度**：本專案前端已定 deploy 至 Cloudflare Pages（SPEC-014 §4）
6. **TypeScript first-class**：SPEC-014 §9 要求所有前端 code 過 `tsc --noEmit`
7. **生態與招募**：Taipei / Taiwan React 工程師供給遠大於 Vue 3 + Nuxt 3
8. **遷移成本**：當前無 production 前端 code，遷移成本 = 0（這是切換的最佳時機）

## 3. Considered Options

### Option A — 維持 Nuxt 3 + Vue 3，放棄 Kumo

採用 Vue 系元件庫如 **shadcn-vue / PrimeVue / Vuetify**，繼續以 Nuxt 3 為框架。

### Option B — Next.js 15 (App Router, React 19) + Cloudflare Kumo ⭐

```
apps/web/            Next.js 15 App Router
  app/
    layout.tsx       RSC root
    page.tsx         landing page (RSC)
    markets/
      [id]/page.tsx  ← 'use client' for WS
  components/        React 函式元件 (.tsx)
  hooks/             use{Name}.ts
  lib/               純函式 utility
```

技術棧：

| 項目 | 選擇 |
|---|---|
| Framework | Next.js 15 (App Router, React 19, RSC) |
| 元件庫 | `@cloudflare/kumo`（建立於 Base UI） |
| Icons | `@phosphor-icons/react` |
| 狀態 | Zustand |
| Data Fetching | TanStack Query v5 |
| 樣式 | TailwindCSS + Kumo CSS variables |
| 表單 | React Hook Form + Zod |
| Charts | Lightweight Charts (TradingView) |
| 部署 | Cloudflare Pages via `@cloudflare/next-on-pages` |

### Option C — Next.js 15 + Radix UI + shadcn/ui（不採用 Kumo）

維持 React 系生態，但用業界更熟悉的 Radix + shadcn/ui 作為元件層。等於否決業主對元件庫的指定。

### Option D — Remix + React + Cloudflare Kumo

Remix 在 nested routing 與 form action 上比 Next.js App Router 更直觀，與 Cloudflare Workers 部署整合度高。

### Option E — SvelteKit + 自刻元件

放棄第三方元件庫，全自刻 + SvelteKit。完整控制 bundle size 與 a11y 行為。

## 4. Decision Outcome

**選擇 Option B：Next.js 15 + React 19 + `@cloudflare/kumo`。**

> ⚠️ **同時明確記錄**：本專案壽命週期內，前端**禁止重新引入 Nuxt 3 / Vue 3 / Pinia / VeeValidate / Composition API**作為主要技術棧。任何「我們混用兩套框架」「先用 Vue 寫 admin、用 React 寫 web」的提案，本 ADR 即為拒絕依據。理由見 §4.1、§4.5。

### 4.1 為何選 B 而非 A（維持 Vue + 換掉 Kumo）

A 的核心問題是**否決業主決策**：

- Kumo 是業主於 2026-05-05 親自指定的元件庫，並提供官方資料來源（`github.com/cloudflare/kumo`、`kumo-ui.com`）
- Tech Lead 的角色是評估「在業主指定下的最佳實作路徑」，不是覆寫業主對元件庫的選擇
- 若 Tech Lead 認為 Kumo 不適合本專案，正確路徑是寫 ADR 提案否決，而非偷渡 Vue 元件庫
- 業主指定的理由（無障礙、Cloudflare 生態、CSS variables 主題化、MIT）皆成立，不存在「Kumo 不適合」的技術論據

A 否決。

### 4.2 為何選 B 而非 C（React 但棄 Kumo）

C 看似折衷——「維持 React 生態、用更主流的 Radix + shadcn/ui」——實則同樣否決業主決策：

| 議題 | Kumo (B) | Radix + shadcn/ui (C) |
|---|---|---|
| 業主指定 | ✅ | ❌（推翻業主選型） |
| 無障礙 | ✅ Base UI | ✅ Radix |
| Cloudflare 生態 | ✅ 官方維護 | ❌ 無關 |
| CSS variables 主題化 | ✅ | △（shadcn/ui 為 copy-paste，主題化需手動）|
| 業界熟悉度 | △（2024 才開源） | ✅（業界主流） |
| 升級保證 | ✅（Cloudflare 維護） | △（shadcn/ui 為 vendor 模式，自負其責） |

「業界熟悉度」差距不足以推翻業主決策。C 否決。

### 4.3 為何選 B 而非 D（Remix + Kumo）

D 在路由與 form 上有設計上的優雅之處，但：

| 議題 | Next.js 15 (B) | Remix (D) |
|---|---|---|
| App Router + RSC 成熟度 | ✅ v15 穩定 | △（Remix 2.x 仍在演進，React Router 7 合併中）|
| Cloudflare Pages 官方 adapter | ✅ `@cloudflare/next-on-pages` | ✅ Remix Cloudflare adapter |
| 招募親和度 | ✅ 業界主流 | △（Remix 工程師供給較少） |
| Kumo 文件 / 範例對齊 | ✅（Cloudflare 內部多用 Next.js） | △ |
| 撮合報價即時性 | ✅ 'use client' + WS | ✅ 同 |

D 並無致命缺陷，但 Next.js 在生態 / 招募 / 文件對齊上較優。D 不採用，但不否決：未來若團隊有強 Remix 偏好，可寫 ADR-XXXX 提案切換。

### 4.4 為何選 B 而非 E（SvelteKit + 自刻）

E 推翻「採用元件庫」這個前提。本 ADR 的範圍是接受業主指定的 Kumo，故 E 不在比較範圍內。若未來業主撤回 Kumo 指定，再評估 E。

### 4.5 為何不混用 Vue + React（拒絕折衷案）

「Web 用 React、Admin 用 Vue」「Landing 用 Vue、Markets 用 React」之類折衷一律否決：

1. **共用程式庫複雜化**：`packages/ts-sdk`（API client、型別、Zod schema）需同時支援兩個 framework 的 binding
2. **設計 token 重複實作**：`theme.css` 餵給 Kumo 的 CSS variables 在 Vue 端無對等元件可套
3. **招募成本翻倍**：需要兩倍工程師熟悉度
4. **CI / lint / test 工具鏈翻倍**：ESLint config、Vitest config、Playwright fixture 都需兩套
5. **bug 表面積非線性增加**：兩套框架交界處（iframe、micro-frontend）是經典 bug 溫床

技術一致性是工程效率的基本盤。混用無正當理由。

## 5. Consequences

### 5.1 Positive

1. **無障礙合規免費獲得**：Base UI 處理 ARIA / focus / 鍵盤，前端合規流程的可及性要求 day-1 達標
2. **Cloudflare 生態一致性**：前端 (Pages) + 元件庫 (Kumo) + Object Store (R2) + WAF / CDN 全在同一供應商，故障排查 / billing / 支援單一窗口
3. **設計 token 易切換**：三個 design 候選（Apple / Airbnb / Lamborghini）對 Kumo 而言只是 `theme.css` 的差異，不需動元件原始碼
4. **TypeScript 與後端 schema 共用**：Zod schema 可由 protobuf 經 protoc-gen-zod 產出，前端 React Hook Form 與後端 Rust（serde + validator）皆消費同一份 .proto 為 source of truth（packages/ts-sdk）
5. **RSC 對 SEO 與初載速度有利**：landing page / 市場列表頁可用 RSC，撮合頁面降到 'use client'
6. **招募親和度高**：Taiwan React 工程師供給遠大於 Vue 3 + Nuxt 3
7. **與 SPEC-014 §9 命名規範一致**：PascalCase `.tsx` + `use{Name}.ts` 為 React 標準，無需發明專案內部命名

### 5.2 Negative

1. **Kumo 仍年輕**：2024 開源，元件覆蓋度可能不及 Radix 全家桶；遇到缺件需降到 Base UI 自組
2. **React 19 + RSC 學習曲線**：'use client' / 'use server' / Server Action 的心智模型對 Vue 工程師需要時間適應
3. **Cloudflare Pages 對 Next.js 的支援有 caveat**：`@cloudflare/next-on-pages` 不支援 Node.js runtime API（須用 Edge runtime），部分 SSR 行為需妥協
4. **PRD / SPEC-014 v0.1.1 已對外溝通 Nuxt 3**：對既有相關方需重新告知，但因無 production code，遷移成本實質為 0
5. **Phosphor Icons 為強制 peer dep**：放棄 lucide / heroicons 等替代 icon 庫的可能性
6. **Kumo 主題化文件尚淺**：CSS variables 對應表可能需要自行實驗摸清

### 5.3 Mitigation Plan

針對 Negative #1（Kumo 元件覆蓋度）：

- 第一個 PR 之前先用 `npx @cloudflare/kumo ls` 列出可用元件，比對 landing page 設計需求
- 缺件清單寫入 `design/COMPONENTS.md` 並評估「自組 vs 等 Cloudflare 補」
- 自組元件統一放 `packages/ui/`，命名 `Proph{Name}` 與 Kumo 區分

針對 Negative #2（RSC 學習曲線）：

- SPEC-014 §8.2 已提供 React + Kumo 範例
- 第一個前端工程師上手時，Tech Lead 要求其先讀 Next.js App Router 官方文件 + Kumo README + Base UI Composing Components 章節
- 第一個 PR 強制 4-eye review 且須 Tech Lead 簽核

針對 Negative #3（Cloudflare Pages Edge runtime 限制）：

- 第一個 deploy 之前測試 SSR + Edge runtime 行為
- Server Action 內禁止使用 Node.js fs / crypto.randomBytes 等 API（改用 Web Crypto）
- 若遇到無法繞過的限制，fallback 至 Vercel deploy（保留選項，不主動切換）

針對 Negative #4（已對外溝通 Nuxt）：

- README / SPEC-014 / `.gitignore` / CLAUDE.md 已同步更新（2026-05-05）
- PRD 待業主決定是否更新

## 6. Compliance & Legal Boundary

本決策強制以下 codebase 紅線：

```
1. 禁止引入 Vue / Nuxt / Pinia / VeeValidate / @vue/* 任何依賴
   - package.json 不應出現 vue、nuxt、@vue/*、pinia、vee-validate
   - CI 加入 dependency 檢查 step

2. 禁止引入第二套 UI 元件庫（與 Kumo 並用）
   - 不可同時使用 Radix UI、shadcn/ui、Mantine、MUI、Chakra UI、Ant Design
   - 唯一例外：Kumo re-export 的 Base UI primitives

3. React 元件必須為 TypeScript 函式元件
   - 禁止 class component（除非為 React error boundary，且必須有註解說明）
   - 所有元件 props 必須有顯式 type / interface
   - 不允許 `any` 進入 production code（lint rule enforced）

4. Server / Client component 邊界須明確
   - 'use client' directive 必須在檔案第一行
   - 禁止在 server component 內 import 純 client lib（如 zustand store、Lightweight Charts）
   - WebSocket 訂閱必須在 client component 內

5. Kumo 元件樣式覆寫走 CSS variables，不走 className 強制覆寫
   - 禁止 `!important`
   - 禁止 deep selector hack（`[class*="kumo-button"]` 之類）
   - 主題切換透過 `theme.css` 變更 `--kumo-*` variables
```

任何違反 → 元件庫紀律破壞 → 升級 Kumo 時相容性失控 → 三套設計 token 無法乾淨切換 → 設計系統失效。

## 7. Validation Criteria

本決策被視為「成功落地」需滿足：

- [ ] `apps/web/` 以 Next.js 15 App Router 建立，可 build 過、可 `pnpm dev` 啟動
- [ ] `package.json` 無任何 Vue / Nuxt / Pinia / VeeValidate 依賴
- [ ] `@cloudflare/kumo` 已安裝且至少一個元件（Button）成功渲染於 `app/page.tsx`
- [ ] `theme.css` 可載入並覆寫至少 `--kumo-color-primary`，視覺上 Button 顏色變動
- [ ] React Hook Form + Zod 範例 form 完成（demo 用 KYC 提交流程）
- [ ] Zustand store 範例完成（demo 用 wishlist 市場清單）
- [ ] TanStack Query 範例完成（demo 用市場列表 fetch）
- [ ] WebSocket 訂閱範例完成（demo 用即時報價推播）
- [ ] 部署至 Cloudflare Pages 成功，SSR 與 client-side hydration 皆正常
- [ ] ESLint + `tsc --noEmit` 在 CI 全綠
- [ ] Lighthouse a11y score ≥ 95（landing page）

## 8. References

### 8.1 內部文件

- [SPEC-014 §3.2 前端](../../../specs/SPEC-014-tech-architecture.html) — 技術選型表（v0.1.2 已更新）
- [SPEC-014 §8.2 React / Next.js 15 範例](../../../specs/SPEC-014-tech-architecture.html)
- [SPEC-014 §9 命名規範](../../../specs/SPEC-014-tech-architecture.html)
- [`design/COMPONENTS.md`](../../../design/COMPONENTS.md) — Kumo 詳細選用文件、技術棧驗證來源
- [`design/landing-apple.html`](../../../design/landing-apple.html) — 候選方案 1 視覺化
- [`design/landing-airbnb.html`](../../../design/landing-airbnb.html) — 候選方案 2 視覺化
- [`design/landing-lamborghini.html`](../../../design/landing-lamborghini.html) — 候選方案 3 視覺化

### 8.2 外部參考

- Cloudflare Kumo — https://github.com/cloudflare/kumo
- Kumo UI 官方文件 — https://kumo-ui.com/
- Base UI（Kumo 底層）— https://base-ui.com/
- Next.js 15 App Router — https://nextjs.org/docs/app
- `@cloudflare/next-on-pages` — Cloudflare Pages 官方 Next.js adapter
- React Hook Form + Zod resolver — https://react-hook-form.com/
- TanStack Query v5 — https://tanstack.com/query/latest
- Zustand — https://github.com/pmndrs/zustand

## 9. Notes for Codex / Future Engineers

當你在 `apps/web/` 或 `apps/admin/` 動手時：

1. **任何 UI 元件優先用 `@cloudflare/kumo`**：找不到再降到 Base UI primitives（Kumo re-export），最後才自組到 `packages/ui/`
2. **不要 import Vue / Nuxt / Pinia 任何套件**：CI 會擋；遇到「我習慣 Vue 寫法」請改寫成 React idiom
3. **`'use client'` 是合約，不是裝飾**：判斷標準是「是否使用 useState / useEffect / 瀏覽器 API / 第三方 client lib」。錯誤標註會在 SSR 階段炸 hydration mismatch
4. **WebSocket 一律在 client component 內訂閱**：透過 custom hook（`useMarketStream`）封裝，禁止在 server component 直接連 WS
5. **元件樣式優先用 Tailwind className + Kumo CSS variables**：不要寫 inline `style={{}}` 除非是動態計算值（如 progress bar 寬度）
6. **表單一律 React Hook Form + Zod**：禁止 `useState` 自管表單狀態；Zod schema 與後端共用，放在 `packages/ts-sdk/schemas/`
7. **狀態管理三層**：URL search params (Next.js `useSearchParams`) → 元件 state (`useState`) → 跨頁面狀態 (Zustand)。禁止用 Zustand 存「只在一個頁面用的狀態」
8. **資料 fetching 走 TanStack Query**：禁止在 useEffect 內直接 fetch；server component 可用 `await fetch()` 但須注意 cache 行為
9. **icon 一律 `@phosphor-icons/react`**：禁止引入 lucide / heroicons / svg sprite 自管
10. **若你認為某個 Kumo 元件不夠用**：先確認是否能用 Base UI 重組，再考慮 `packages/ui/Proph{Name}.tsx`，不要 fork Kumo 原始碼
11. **若你認為本 ADR 的決策應該被推翻**：寫一份 ADR-XXXX 標記本 ADR 為 Superseded，附上替代方案的論證、遷移計畫、業主簽核——不要直接改 `package.json` 引入 Vue

## 10. Pre-Execution Validation Items（動工前驗證項目）

> **MADR 體裁註記**：本節非「待重新決定的問題」（ADR 已 Accepted，React + Kumo 為架構定案）。以下為動工前必須驗證的實作細節，由 `apps/web/` 第一個 PR 之前釐清。所有跨 SPEC 的決策議題改由 [SPEC-000 §5](../../../specs/SPEC-000-index.html) 中央追蹤表收斂。

| ID | 待驗證項目 | 驗證對象 | 影響 |
|----|----------|---------|------|
| V-1 | Kumo 當前元件清單比對三個 landing page 的元件需求，列出缺件 | Tech Lead + 前端工程師 | 缺件需評估自組 / 等 Cloudflare 補 / 用 Base UI 組 |
| V-2 | `@cloudflare/next-on-pages` 對 Server Action / streaming SSR 的支援限制 | DevOps + 前端 | 影響 SSR 策略；若限制過多需評估改 Vercel |
| V-3 | Kumo CSS variables 對應表（哪些 `--kumo-*` 可被覆寫到主題化） | 前端 + 設計師 | 影響三個 design token → `theme.css` 的對應策略 |
| V-4 | Edge runtime 對 Node.js API 的限制清單，與 ts-sdk 內建議 | 後端 + 前端 | 影響 ts-sdk 共用程式庫的 runtime 限制 |
| V-5 | LIFF SDK（LINE）的 React wrapper 評估（官方 SDK 為 vanilla JS） | 前端 | 影響 `apps/liff/` 的實作路徑 |

V-1 ~ V-3 標記為 **must-resolve**（影響第一個 `apps/web/` PR）。V-4 ~ V-5 標記為 **defer-acceptable**（可在第一次跨服務 / LINE 整合需求出現前處理）。

## 11. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-05 | v1.0 | Tech Lead | 首版 Accepted。基於業主同日指定 Kumo 為元件庫之決策，論證並選定 Next.js 15 + React 19 為配套框架。同步更新 SPEC-014 v0.1.2、`design/COMPONENTS.md`、`README.md`、`.gitignore`、`CLAUDE.md` PR checklist。 |
