# ADR-0008 — 利益邏輯一律由 Rust 後端權威執行，前端驗證僅作 UX 輔助

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted |
| **Date** | 2026-06-16 |
| **Deciders** | Founder, Tech Lead |
| **Consulted** | Codex 開發代理 |
| **Informed** | 全工程團隊、Codex 開發代理 |
| **Tags** | security · backend · frontend · ledger · payments · core-architecture |
| **依賴** | ADR-0003, ADR-0005, ADR-0006, ADR-0007, SPEC-002, SPEC-003, SPEC-010, SPEC-011, SPEC-016, SPEC-024 |
| **影響** | AGENTS.md, CLAUDE.md, frontend/backend trust boundary |

---

## 1. Context and Problem Statement

Prophetica 的核心風險集中在積分、金幣、金流、兌換、佣金、限額、帳本與稽核狀態。這些狀態如果被前端驗證、Next.js route handler、或其他可被客戶端影響的邏輯決定，攻擊者可以透過改封包、改 JS、重放請求、跳過 UI 流程、或偽造 client-side state 繞過規則。

本 ADR 把信任邊界明文化為不可放寬的架構原則：

> 永遠不要相信前端的任何驗證，所有牽涉到利益（不管是積分、金幣、金流）的邏輯，一律放在 Rust 後端。

## 2. Decision Drivers

1. **客戶端不可被信任**：瀏覽器、LIFF WebView、Next.js client state、localStorage、表單欄位、query string、cookie 以外的 client-provided value 都可被使用者修改。
2. **利益狀態需要可稽核**：任何 balance、ledger、payment、commission、redemption、refund、limit 的變化都必須有 Rust backend service、ledger transaction、database invariant、audit log 或 idempotency key 支撐。
3. **既有架構已選 Rust 後端**：ADR-0005 已決定後端語言為 Rust；利益邏輯應放在 services crate，而不是前端 app。
4. **帳本 invariant 是最後防線**：ADR-0006 / ADR-0007 已把 wallet outflow、redemption 同人綁定下沉到 DB trigger / ledger invariant；本 ADR 將同一精神擴展到所有利益敏感邏輯。
5. **降低未來 drift**：前端可顯示、預檢、禁用按鈕、提供錯誤提示，但不得成為唯一 enforcement。

## 3. Considered Options

### Option A — 前端與後端各自驗證，視功能方便決定權威端

**否決理由**：權威位置不固定會造成 drift。攻擊者只需要找到一個沒有後端 enforcement 的路徑，就能繞過 UI。

### Option B — Next.js route handler 可作利益邏輯決策

**否決理由**：Next.js route handler 可作 BFF/proxy/session header 補齊，但本專案的利益邏輯、帳本、金流與風控權威必須集中於 Rust service crate。把權威邏輯放在 TypeScript BFF 會繞開 ADR-0005 與 ledger invariant 的治理方式。

### Option C — Rust 後端為唯一權威，前端驗證僅作 UX 輔助 ⭐

前端與 Next.js route handler 可做：
- 顯示後端回傳的 balance、limit、status、eligibility
- 做 non-authoritative 預檢以改善 UX
- 轉送 session、CSRF、traceparent、locale 等 request context
- 顯示後端拒絕原因

前端與 Next.js route handler 不可做：
- 決定使用者是否取得、失去、轉移、鎖定、解鎖、兌換、退款、結算或提領任何利益
- 相信 client-provided balance、price、fee、limit、tier、role、commission rate、payment amount、redemption entitlement
- 只靠 UI disabled state、form validation、client-side schema 或 route handler if statement 保護利益敏感操作

## 4. Decision Outcome

**選擇 Option C。**

所有牽涉利益的邏輯一律放在 Rust 後端 services crate，並在必要時由 ledger/database invariant 強制。Frontend validation 只能作 UX hint；Next.js route handler 只能作 BFF/proxy 或 session/context adaptation，不得作為利益敏感規則的最終裁決點。

利益敏感範圍至少包含：
- points / coins / balance / locked / frozen / entitlement
- order placement、matching、settlement、refund、chargeback
- top-up、payment webhook、real-money enablement
- redemption、entitlement fulfillment、expiry、cancel
- affiliate commission、manual points grant/deduct、support compensation
- KYC/AML tier 對金額、交易、儲值、提款或兌換限額的影響
- audit state、4-eye approval、idempotency、risk hold、account freeze

## 5. Consequences

### 5.1 Positive

1. **信任邊界清楚**：前端可被繞過不再影響資產安全；Rust backend 是唯一權威。
2. **審計更簡單**：利益變動集中在 Rust service、ledger transaction、DB invariant、audit log。
3. **降低重放與竄改風險**：client-provided values 必須由後端重新查詢或重新計算。
4. **與既有 ledger ADR 對齊**：ADR-0006 / ADR-0007 的 zero-trust 原則被擴展到全域利益邏輯。

### 5.2 Negative

1. **前端迭代不能單獨交付利益功能**：任何利益敏感 UX 變更都需要確認 Rust backend contract。
2. **BFF route 需要保持薄層**：TypeScript route handler 不能因方便而累積業務裁決。
3. **測試成本增加**：利益敏感流程至少需要 Rust backend 測試；前端測試只能證明 UX，不足以證明安全。

### 5.3 Mitigation Plan

- 新增或修改利益敏感流程時，PR 必須指出對應 Rust service crate、ledger/database invariant、audit log 或 idempotency enforcement。
- 前端 PR 若新增 points/coins/payment/redemption/commission/limit 類 UI，review 必須檢查是否只展示或 proxy 後端狀態。
- 若暫時只能做 mock/demo，UI 文字與程式路徑必須明確標示為 non-authoritative mock，且不得連到真實利益狀態變更。

## 6. Compliance & Security Boundary

任何將利益敏感權威邏輯放在前端或 Next.js route handler 的 change，預設視為架構違規與安全風險。除非另有 Accepted ADR 明確放寬，否則 code review 必須要求改回 Rust backend enforcement。

## 7. Validation Criteria

本 ADR 落地與後續維護需滿足：

- [x] `AGENTS.md` 記錄本規則，供 Codex 開發代理優先遵守。
- [x] `CLAUDE.md` 記錄同一信任邊界，供 Claude Code 開發代理遵守。
- [ ] 前端掃描不得發現可直接改變 points/coins/payments/redemption/commission/limits 的權威邏輯。
- [ ] Rust 後端存在 wallet ledger、matching、payment status、redemption entitlement、commission/support compensation 等利益敏感流程的權威 enforcement。
- [ ] 新增利益敏感功能時，至少有 Rust backend test 或 DB invariant test；前端測試不得作為唯一驗證。

## 8. References

- [ADR-0003 — 採用雙簿記帳本](./ADR-0003-double-entry-ledger.md)
- [ADR-0005 — 後端語言由 Go 切換至 Rust](./ADR-0005-backend-language-rust-over-go.md)
- [ADR-0006 — 帳本不變式集合擴展：INV-6](./ADR-0006-ledger-invariant-set-extension-inv6.md)
- [ADR-0007 — INV-6 redemption carve-out and INV-7](./ADR-0007-inv6-redemption-carveout-and-inv7.md)
- [SPEC-002 — Funds Safety](../../../specs/SPEC-002-funds-safety.html)
- [SPEC-016 — Payment Gateway](../../../specs/SPEC-016-payment-gateway.html)
- [SPEC-024 — Redemption Entitlement](../../../specs/SPEC-024-redemption-entitlement.html)

## 9. Notes for Codex / Future Engineers

Before implementing or reviewing any feature touching 積分、金幣、金流、兌換、佣金、限額、帳本、稽核狀態:

1. Find the Rust backend authority first.
2. Treat all frontend values as attacker-controlled.
3. Use frontend validation only for UX.
4. Require Rust backend tests or DB invariant tests for enforcement.
5. If no backend authority exists, stop and add one before shipping frontend behavior.

## 10. Revision History

| Version | Date | Change |
|---------|------|--------|
| v1.0.0 | 2026-06-16 | Accepted initial server-authoritative benefit logic rule. |
