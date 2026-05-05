# SPEC-000 — 規格總索引（Master Index）

> **prophetica** — 趣味預測娛樂遊戲平台
> 文件版本：v0.1.4 · 最後更新：2026-05-05 · 維護人：Tech Lead

---

## 0. 文件性質

本目錄下所有 `SPEC-*.md` 文件均為**可實施規格**（Implementable Specification），遵循 SDD（Spec-Driven Development）方法論。

- 每份 SPEC 都是 Codex 開發任務的輸入。
- 所有實作 PR 必須在 description 中引用 `SPEC-XXX § N.N` 章節。
- 任何 SPEC 變更須先提 PR 審閱，再進行對應實作。
- SPEC 與 code 同步進入 git，不允許 SPEC 落後於程式碼。

## 1. 專案定位

| 項目 | 內容 |
|------|------|
| 中文名稱 | 趣味預測競猜娛樂遊戲平台 |
| 英文代號 | prophetica |
| 法律分類 | **娛樂遊戲服務**（非博弈、非彩券、非衍生性金融商品） |
| 玩家行為 | 以站內**虛擬點數**對未公開事件結果進行預測，與其他玩家撮合 |
| 平台角色 | **中立撮合平台**，僅收手續費，不持有合約倉位、不做造市商 |
| 出金政策 | **台灣司法管轄區內無任何出金通道**；點數僅可在站內使用，不可兌換現金 / 虛擬貨幣 / 實體商品 |
| 主要市場 | 亞太娛樂用戶 |
| 嚴格 Geo-block | 以 [SPEC-001 §2](./SPEC-001-legal-compliance.md#2-geo-block-矩陣) 的 🔴 紅名單機制與多層防禦為準；首發保守名單待法律意見書最終確認 |

## 2. SPEC 清單

### P0 — 必須在第一行 code 之前完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-001](./SPEC-001-legal-compliance.md) | 法律合規與司法管轄框架 | 合規三原則、Geo-block、VPN 政策、ToS、監管稽核資料、隱私與 AML | 🟢 v0.1.1 |
| [SPEC-002](./SPEC-002-funds-safety.md) | 資金安全與虛擬點數帳本 | 雙簿記、隔離帳戶、對帳機制 | 🟢 v0.1 |
| [SPEC-003](./SPEC-003-matching-engine.md) | 撮合引擎詳規 | Order Book 資料結構、訂單類型、撮合演算法、API、SLO | 🟢 v0.1 |
| [SPEC-004](./SPEC-004-settlement-dispute.md) | 結算與爭議處理 | Oracle、結算狀態機、申訴窗口 | 🟢 v0.1 |
| [SPEC-015](./SPEC-015-market-lifecycle.md) | 市場生命週期管理 | 市場創建、上架審查、內容紅線、開盤排程、強制下架 | 🟢 v0.1 |
| [SPEC-016](./SPEC-016-payment-gateway.md) | 支付通路整合 | 通路選型、Webhook、Chargeback、跨幣別、PCI 範圍 | 🟢 v0.1 |

### P1 — 必須在公開 Beta 之前完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-005](./SPEC-005-kyc-aml.md) | KYC/AML 整合 | 分層 KYC、SaaS 整合、制裁名單篩查 | 🟢 v0.1 |
| [SPEC-006](./SPEC-006-responsible-gaming.md) | 責任博弈機制 | 自我排除、限額、冷靜期 | 🟢 v0.1 |
| [SPEC-007](./SPEC-007-observability.md) | 可觀測性與監控 | Metrics/Log/Tracing、SLO、告警 | 🟢 v0.1 |
| [SPEC-008](./SPEC-008-disaster-recovery.md) | 災備與高可用 | RTO/RPO、跨區備援 | 🟢 v0.1 |
| [SPEC-009](./SPEC-009-dmm-liquidity.md) | DMM 與流動性 | 做市商計畫、API、rebate | 🟢 v0.1 |
| [SPEC-010](./SPEC-010-security-antifraud.md) | 安全與反作弊 | 裝置指紋、行為分析、洗售偵測 | 🟢 v0.1 |

### P2 — 公開 Beta 後 90 天內完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-011](./SPEC-011-affiliate-commission.md) | 代理分潤體系 | 多層代理、佣金計算、稅務扣繳 | 🟢 v0.1 |
| [SPEC-012](./SPEC-012-customer-support.md) | 客服與爭議升級 | SLA、工單系統 | 🟢 v0.1 |
| [SPEC-013](./SPEC-013-cicd-release.md) | CI/CD 與發布 | Pipeline、Feature Flag、回滾 | 🟢 v0.1 |

### 跨章節文件

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-014](./SPEC-014-tech-architecture.md) | 整體技術架構 | 服務拓撲、技術棧、Repo 結構 | 🟢 v0.1 |

## 3. 全域名詞表（Glossary）

| 名詞 | 中文 | 定義 |
|------|------|------|
| Market | 市場 / 合約 | 一個可預測的未公開事件，含 YES/NO 兩個 Outcome |
| Outcome | 結果腿 | 市場的兩種結果之一（YES 或 NO） |
| Order | 訂單 | 玩家對某個 Outcome 在特定價格下單的意圖 |
| Trade | 成交 | 兩張對手方訂單撮合形成的交易紀錄 |
| Position | 持倉 | 玩家在某個 Outcome 上的累計持有股數 |
| Settlement | 結算 | 市場到期後依官方判定結果，將持倉轉為點數 |
| Tick | 最小價格單位 | 1 cent（0.01 點） |
| Lot | 最小數量單位 | 1 股 = 1 點面額 |
| Points | 站內點數 | 玩家儲值購買的虛擬遊戲點數，**不可兌回現金** |
| DMM | Designated Market Maker | 受邀的外部做市商 |
| KYC | Know Your Customer | 客戶身份驗證 |
| AML | Anti-Money Laundering | 反洗錢機制 |

## 4. 變更紀錄

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-05 | v0.1.4 | Tech Lead | 第二輪交叉一致性 review（純技術視角，合規由業主自治）：(1) SPEC-005 §2.4 對 SPEC-001 §3.1 之 dead link 改為指 SPEC-001 §2.1；(2) SPEC-015 §11 7 年保留改為呼應 SPEC-001 §1.1 ③；(3) ADR-0002 多處對 SPEC-001 §2 / §2.2 / §8.3 / §10、SPEC-002 §2.1 之 dead link 全部修正並補 v1.0.2 Revision History；(4) 補回 SPEC-002 §3.2 內部精度（純技術整數化規範）；(5) SPEC-016 §7 跨幣別整節重寫，明確 Phase 1 單幣別 / Phase 2 多幣別保留設計，移除 `Bank.TrustAccount.USD` 舉例；(6) SPEC-002 §12 Open Questions 補齊編號 1/2 並收斂跨檔議題引用。 |
| 2026-05-05 | v0.1.3 | Tech Lead | 依 SPEC-001 v0.1.1 精簡後章節重整索引：更新 SPEC-001 標題、摘要、狀態版本；Geo-block 全域描述改以 SPEC-001 §2 為準；跨檔決策追蹤表中 SPEC-001 章節引用改為現行 §2 / §3.1 / §4.1，移除對已刪除公司結構與司法管轄區分級表的硬引用。 |
| 2026-05-05 | v0.1.2 | Tech Lead | 完成首輪交叉一致性 review：(1) 修 SPEC-002 §5 INV 編號對齊 ADR-0003 §5；(2) 修 PRD.md dead link；(3) HK/SG geo-block 統一為保守 🔴；(4) 修 SPEC-002 §4.4.4 零金額結算 entry 衝突；(5) ADR-0002 / ADR-0003 §11 改名為「Pre-Execution Validation Items」；(6) §5 改為跨檔決策追蹤表（含 Resolved-by 欄位）；(7) SPEC-005 補 declared_monthly_income 並對齊 SPEC-006 §4.1；(8) SPEC-005 §2.4 補首發 5 國 Tier 1 證件驗證；(9) 新增 SPEC-015 市場生命週期；(10) 新增 SPEC-016 支付通路整合；(11) SPEC-003 §12.1 補 PLATFORM_SYSTEM_USER_IDS sentinel 定義；(12) SPEC-015 / SPEC-016 加入 P0 索引。 |
| 2026-05-05 | v0.1.1 | Tech Lead | 專案英文代號定案：propheticaNet（暫定）→ prophetica；同步刪除 legacy PRD.md（內容與 SPEC-000 / SPEC-001 多處衝突）；連帶 bump SPEC-001 / SPEC-005 / SPEC-014 至 v0.1.1（修正內文專案名稱）；ADR-0001 / ADR-0002 同步修名但依 ADR 慣例不變動 Status / Date。 |
| 2026-05-05 | v0.1.0 | Tech Lead | 初版建立，14 份 SPEC 全數草稿完成 |

## 5. 跨檔決策追蹤表（Cross-cutting Decision Tracker）

> 本表為跨多份 SPEC / ADR 的全域決策點之**單一真相來源**。任何議題一旦被某 SPEC / ADR 決定，**Status** 改為 `Resolved` 並填入 `Resolved by` 欄位；其他文件的 Open Questions 不重複記錄此議題。

| ID | 議題 | 影響範圍 | Status | Resolved by | 待動作 |
|----|------|---------|--------|------------|--------|
| Q-1 | 控股公司司法地：BVI vs Cayman vs Delaware | ADR-0002 | ✅ Resolved | [ADR-0002 §4.4](../docs/architecture/decisions/ADR-0002-three-company-corporate-structure.md) | BVI；未來 IPO 路徑前重組為 Cayman/Delaware |
| Q-2 | Op Co. 執照地：Malta MGA / Curaçao / PAGCOR / Isle of Man | ADR-0002 §11 V-1 | 🟡 Pending | — | 動工前由執照地律師擇一，影響 Op Co. 設立地 |
| Q-3 | 點數計價單位：1 點 = TWD 1 vs USD 0.01 vs 抽象單位 | SPEC-002 §3.1 / ADR-0003 §11 V-5 | 🟡 Pending（暫定 TWD 1:1）| SPEC-002 §3.1 列暫定方案 | 國際市場入金前最終敲定 |
| Q-4 | 入金通路供應商：綠界 / 藍新 / 紅陽 / Stripe；MCC 接受度 | SPEC-001 §2.1 / SPEC-016 | 🟡 Pending | [SPEC-016 §3.2](./SPEC-016-payment-gateway.md) 列驗證項 | 簽通路合約前完成至少 3 家詢價，並對齊支付 Gateway BIN 檢查 |
| Q-5 | 未成年下限：18 歲 vs 20 歲 | SPEC-001 §3.1 / SPEC-006 §6 | ✅ Resolved | [SPEC-006 §6](./SPEC-006-responsible-gaming.md) | 18 歲（依台灣民法成年新制）|
| Q-6 | 數據主權：Cloudflare R2（EU / 全球）vs Linode Object Storage (Tokyo) vs AWS S3 (Singapore) | SPEC-001 §4.1 / SPEC-005 §5.1 | 🟡 Pending | — | 個資跨境傳輸合規評估；目前 SPEC-001 §4.1 指定 KYC 證件在 R2 EU、撮合事件流在 Linode Tokyo |

### 5.1 議題狀態圖示

| 圖示 | 含義 |
|------|------|
| ✅ Resolved | 已被某 SPEC / ADR 拍板 |
| 🟡 Pending | 待決定，但不阻塞動工 |
| 🔴 Deal-breaker | 阻塞 P0 收尾，必須在第一行 code 之前解決 |

### 5.2 維護規則

- 新增跨檔議題請寫成新一列；**不要**在個別 SPEC 的 Open Questions 重複記錄已在本表的議題
- 議題被拍板後，將相關 SPEC 的 Open Questions 條目刪除並改為「見 SPEC-000 §5 Q-N」
- ADR 體裁中已 Accepted 的 ADR，§11 採用 `Pre-Execution Validation Items` 命名（驗證項目而非待決議題），參見 ADR-0002 / ADR-0003
