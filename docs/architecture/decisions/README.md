# Architecture Decision Records (ADR)

> 紀錄影響整體架構走向的重大決策，以及其背後的論證與被駁回的替代方案。

## 為什麼需要 ADR

SPEC 描述「**做什麼 / 怎麼做**」，會隨時間演化更新。
ADR 描述「**為什麼這樣決定 / 為什麼別的方案被否決**」，一旦 Accepted **不修改**。

當未來有人（包括你自己）提問「為什麼當時選 X 而不是 Y」，ADR 就是書面答案。

當監管、創投、新進工程師質疑某項架構選擇時，ADR 是不需要重新辯論的依據。

## ADR 撰寫慣例

- 採 **MADR 4.0 格式**（Markdown Any Decision Record）
- 檔名：`ADR-{4位數編號}-{kebab-case-簡述}.md`
- 編號嚴格遞增、不重用、即使 ADR 被 Superseded 編號仍保留
- 一個 ADR 處理**一個**決策，不打包多項

## ADR 狀態

| Status | 含義 |
|--------|------|
| `Proposed` | 草擬中，正在徵求意見 |
| `Accepted` | 拍板，作為當前架構基準 |
| `Deprecated` | 不再推薦，但仍在 codebase 中存在 |
| `Superseded by ADR-XXXX` | 被新的 ADR 取代 |

## 修訂規則

- `Accepted` 後不可直接修改 ADR 主體（除拼寫錯誤等）
- 需要變更決策 → 寫一份新的 ADR，舊 ADR 標記為 `Superseded by ADR-XXXX`
- Revision History 區塊只記錄 metadata 級變更（typo、外部連結更新）

## 目前 ADR 索引

| 編號 | 標題 | 狀態 | 領域 |
|------|------|------|------|
| [ADR-0001](./ADR-0001-order-book-with-dmm-over-lmsr.md) | 採用 Order Book + DMM，捨棄 LMSR 與平台造市腳本 | Accepted | matching-engine, liquidity, regulatory |
| [ADR-0002](./ADR-0002-three-company-corporate-structure.md) | 採用三公司架構（BVI Holdings + 離岸 Op Co. + 台灣 Tech Co.）| Accepted（待律師背書）| corporate-structure, legal, tax, regulatory |
| [ADR-0003](./ADR-0003-double-entry-ledger.md) | 採用雙簿記帳本，捨棄單簿記與「balance += amount」捷徑 | Accepted | ledger, finance, audit, core-architecture, invariant |

## 何時要寫 ADR

- 跨多份 SPEC 影響的選擇（例：撮合演算法影響 SPEC-003 / SPEC-009 / SPEC-001）
- 拒絕了業界常見方案（要解釋「為什麼不」）
- 監管 / 法律相關的核心定位
- 主要技術棧選擇（語言、資料庫、雲服務商）
- 安全與合規邊界
- 拒絕 PRD 中某項功能 / 設計

## 何時**不**需要 ADR

- 程式內部實作細節（屬於 code review 範疇）
- 單一 SPEC 內部章節之間的取捨
- 個別函式 / 類別命名
- 暫時性的 workaround（屬於 code comment 範疇）
- CI 配置調整

## 模板

撰寫新 ADR 時可從 ADR-0001 直接複製欄位結構。核心欄位：

- 表頭 metadata（Status / Date / Deciders / Consulted / Informed / Tags）
- 1. Context and Problem Statement
- 2. Decision Drivers
- 3. Considered Options
- 4. Decision Outcome（含為何選此案、為何不選其他）
- 5. Consequences（Positive / Negative / Mitigation）
- 6. Compliance & Legal Boundary（如適用）
- 7. Validation Criteria
- 8. References
- 9. Notes for Codex / Future Engineers
- 10. Revision History
