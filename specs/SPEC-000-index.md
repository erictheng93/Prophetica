# SPEC-000 — 規格總索引（Master Index）

> **propheticaNet** — 趣味預測娛樂遊戲平台
> 文件版本：v0.1.0 · 最後更新：2026-05-05 · 維護人：Tech Lead

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
| 英文代號 | propheticaNet（暫定） |
| 法律分類 | **娛樂遊戲服務**（非博弈、非彩券、非衍生性金融商品） |
| 玩家行為 | 以站內**虛擬點數**對未公開事件結果進行預測，與其他玩家撮合 |
| 平台角色 | **中立撮合平台**，僅收手續費，不持有合約倉位、不做造市商 |
| 出金政策 | **台灣司法管轄區內無任何出金通道**；點數僅可在站內使用，不可兌換現金 / 虛擬貨幣 / 實體商品 |
| 主要市場 | 亞太娛樂用戶 |
| 嚴格 Geo-block | 美國全境、中國大陸、加拿大 Ontario、法國、新加坡、香港（首發保守名單，待法律意見書最終確認） |

## 2. SPEC 清單

### P0 — 必須在第一行 code 之前完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-001](./SPEC-001-legal-compliance.md) | 法律合規與司法管轄 | 公司結構、Geo-block、ToS/Privacy、責任歸屬 | 🟢 v0.1 |
| [SPEC-002](./SPEC-002-funds-safety.md) | 資金安全與虛擬點數帳本 | 雙簿記、隔離帳戶、對帳機制 | 🟢 v0.1 |
| [SPEC-003](./SPEC-003-matching-engine.md) | 撮合引擎詳規 | Order Book 資料結構、訂單類型、撮合演算法、API、SLO | 🟢 v0.1 |
| [SPEC-004](./SPEC-004-settlement-dispute.md) | 結算與爭議處理 | Oracle、結算狀態機、申訴窗口 | 🟢 v0.1 |

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
| 2026-05-05 | v0.1.0 | Tech Lead | 初版建立，14 份 SPEC 全數草稿完成 |

## 5. 待解決議題（Cross-cutting Open Questions）

> 這些是跨多份 SPEC 的全域決策點，需要在 P0 完成前由業主拍板。

1. **公司註冊地最終定案**：Malta MGA（最嚴格、保護最強，年費 ~EUR 25k）vs Curaçao（彈性、約 USD 25k）vs PAGCOR（菲律賓本土門檻高，但僅限菲律賓玩家）vs Isle of Man（聲譽佳，較貴）。
2. **入金通路供應商**：是否使用台灣本地金流（綠界 / 藍新 / 紅陽）？這些供應商是否可接受「娛樂遊戲」MCC？需確認。
3. **點數計價單位**：是否與 TWD 1:1（容易理解但易被誤認為現金）？或設計為 100 點 = TWD 1 等抽象比例？
4. **未成年防護下限**：18 歲？20 歲（依台灣 民法成年年齡新制 18 歲，但博弈習慣仍用 20 歲）？
5. **數據主權與儲存位置**：Cloudflare R2（全球）vs Linode Object Storage（Tokyo）— 涉及個資跨境傳輸合規。
