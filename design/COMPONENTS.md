# 專案元件庫 — Cloudflare Kumo

> 本專案前端 production code 的 UI 元件統一採用 **Cloudflare Kumo**。
> 三個 landing page 設計檔（`DESIGN-apple.md` / `DESIGN-airbnb.md` / `DESIGN-lamborghini.md`）描述的是「視覺語言 / design tokens」，最終實作會把這些 token 套用到 Kumo 元件的 CSS variables 上。

---

## 來源

| 項目 | URL |
|---|---|
| GitHub repo | https://github.com/cloudflare/kumo |
| 官方文件 / Live docs | https://kumo-ui.com/ |
| 套件名稱 | `@cloudflare/kumo` |
| License | MIT |

## 技術棧（已驗證）

- **框架**：**React**（peer deps：`react`、`react-dom`）— 官方 README 範例：`import { Button, Input, Dialog } from "@cloudflare/kumo"`
- **語言**：**TypeScript** 為主（repo 84.8%），另含 MDX / Astro / CSS
- **底層**：Base UI（headless React primitives — 鍵盤導航 / focus management / ARIA 全自動處理）
- **Icon set**：Phosphor Icons (`@phosphor-icons/react`)
- **設計工具整合**：官方提供 Figma plugin

> **驗證來源**：2026-05-05 直接讀取 https://github.com/cloudflare/kumo 的 README + package.json。
> repo 內**零 Vue / Nuxt / Svelte / web component** 依賴或範例。Kumo 為 React-only。
>
> **本專案因此於同日決定**：前端框架由 Nuxt 3 (Vue 3) 切換至 **Next.js 15 (React 19, App Router)**，以與 Kumo 對齊。詳見 `specs/SPEC-014-tech-architecture.md` v0.1.2 變更日誌。

## 安裝

```bash
pnpm add @cloudflare/kumo
# peer deps
pnpm add react react-dom @phosphor-icons/react
```

列出所有元件（CLI）：

```bash
npx @cloudflare/kumo ls
```

## 為什麼選 Kumo

| 評估點 | 說明 |
|---|---|
| **無障礙開箱即用** | Base UI 處理 ARIA / 焦點 / 鍵盤 — 對 SPEC-001 §X 法遵可及性要求是天然加分 |
| **語意化色彩 token** | 容易把三套 design system 的 palette 對應到 Kumo CSS variables，不用魔改元件原始碼 |
| **Tree-shaking** | 細粒度匯入，bundle size 友善 — 對應 SPEC-014 §X 對前端載入時間的要求 |
| **Re-export Base UI primitives** | 進階情境（自訂 dropdown、複雜 form）可繞過 Kumo 直接用 Base UI |
| **MIT License** | 商用零障礙 |

## 已知元件（隨 Kumo 釋出滾動更新）

文件提到的範例元件：`Button`、`Input`、`Dialog`。完整清單請以 `npx @cloudflare/kumo ls` 為準。

## 設計 token 對應策略

三個 design 檔（Apple / Airbnb / Lamborghini）內的 tokens 不是要疊加，是**三個候選方案**讓業主視覺化挑選。最終定稿後：

1. 把 chosen design 的 `colors` / `typography` / `rounded` / `spacing` / `components` 區塊轉成 `:root { --... }` CSS custom properties
2. 撰寫 `theme.css` 把這些 properties 餵給 Kumo 元件對應的 CSS variables（Kumo 元件本身就是用 CSS variables 暴露樣式）
3. 三套 token 之間切換，理論上只需切換一份 `theme.css`

## 視覺化檢查檔

三個 landing page 在同目錄下：

- `landing-apple.html` — 美術館式安靜留白
- `landing-airbnb.html` — 溫暖消費者市場
- `landing-lamborghini.html` — 黑金劇場感

可雙擊用瀏覽器直接開啟，不需 dev server。最終 production 時這些頁面會由 Kumo React 元件重寫。

## 待辦

- [ ] 業主從三個 landing page 中挑定一個美學方向
- [ ] 把 chosen design 的 tokens 轉為 `theme.css`
- [ ] 建立 `pnpm` workspace、安裝 Kumo
- [ ] 用 Kumo Button / Input / Dialog 重寫 chosen landing page，作為 production 起點
