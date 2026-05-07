# ADR-0001 — 採用 Order Book + DMM，捨棄 LMSR 與平台造市腳本

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted |
| **Date** | 2026-05-05 |
| **Deciders** | Tech Lead, Founder |
| **Consulted** | Compliance Counsel（待法律意見書最終覆核）、首批 DMM 候選人（待招募）|
| **Informed** | 全工程團隊、Codex 開發代理、客服與行銷主管 |
| **Tags** | matching-engine · liquidity · regulatory · core-architecture |

---

## 1. Context and Problem Statement

prophetica 需要為玩家提供一個進行二元（YES/NO）預測下注的市場機制。預測市場業界主流設計可歸為兩派：

- **AMM 派**（Polymarket、Augur、早期 Predictit 部分機制）：以**自動造市公式**（多數採 LMSR — Logarithmic Market Scoring Rule）由平台或合約池主動掛價，玩家對「池」下注。
- **Order Book 派**（Kalshi、傳統證交所）：玩家對玩家**撮合**，平台維護訂單簿，僅收手續費。

兩者各有技術優勢與明確的法律涵義。在我們動工之前必須拍板，因為這個選擇會：

1. 決定平台的**法律地位**（中性中介 vs 交易對手）
2. 決定**撮合引擎核心架構**（純記帳 + 撮合 vs AMM + 流動性池管理）
3. 決定**初期資本曝險**（平台是否要擔負造市責任）
4. 決定**冷啟動流動性策略**（自動有 vs 需要外部來源）

> 原 PRD（PRD.md §3.2，該檔案於 2026-05-05 隨 SPEC v0.1.1 升級時隨同刪除，可由 git history 回溯）提出的「概率對稱腳本」實質為**平台自建造市機器人**——技術屬性接近 LMSR，法律屬性與 AMM 等同。本 ADR 一併處理該方案的去留。

## 2. Decision Drivers

依重要性遞減排序：

1. **不可下場原則**（SPEC-001 §1.1 第②條）：平台不得持有合約倉位、不得在自家訂單簿掛單、不得從玩家輸贏中獲利。
2. **可被監管稽核**：方案必須在執照地（Malta MGA / Curaçao / PAGCOR）申照面試時可被一句話解釋，且不需依賴「但是其實我們有但是…」的詭辯。
3. **流動性**：玩家點開平台時必須看到可下單的市場，不能空盤。
4. **開發與維運成本**：MVP 必須在合理時程上線。
5. **玩家 UX**：點開市場、立刻能下、立刻知道幾率，不需等待。
6. **長期擴展性**：能支援從十個熱門市場擴展到上千個長尾市場。

## 3. Considered Options

### Option A — 純 LMSR（AMM）

平台作為唯一造市商，依 LMSR 公式對任何下注報價：

```
C(q) = b * ln(exp(q_yes / b) + exp(q_no / b))
price_yes = exp(q_yes / b) / (exp(q_yes / b) + exp(q_no / b))
```

`b` 為流動性參數，平台需投入相應資本作為 AMM 保證金。

### Option B — 概率對稱腳本 / 平台造市機器人

維持 Order Book 形式，但平台部署 bot 持續在訂單簿兩側掛單，維持買賣價差在合理範圍。原 PRD 提案。

### Option C — 純 Order Book，無任何造市協助

完全 P2P，平台不介入流動性。

### Option D — Order Book + DMM 計畫 + Liquidity Mining ⭐

主架構為 Order Book，冷啟動透過：

- **Phase 1**：邀請 3–5 家 **外部** DMM（Designated Market Maker），與 Operating Co. 簽訂做市合約，承擔自身做市風險
- **Phase 2**：在反洗售（wash trading）偵測系統成熟後，啟用 Liquidity Mining，從手續費收入中提撥獎勵給為訂單簿提供流動性的一般玩家

平台**任何時候都不**在自家訂單簿掛單。

## 4. Decision Outcome

**選擇 Option D：Order Book + DMM (+ 後續 LM)。**

> ⚠️ 同時明確記錄：**Option A、B 在本專案壽命週期內永久禁止採用**。任何後續若有人提出「就用 LMSR 解決流動性吧」「我們寫個 bot 維持價差就好」，本 ADR 即為拒絕依據。

### 4.1 為何選 D 而非 A（LMSR）

| 比較面向 | LMSR | Order Book + DMM |
|---------|------|-----------------|
| 平台法律地位 | **平台 = AMM = 莊家** | 平台 = 中性撮合場 |
| 玩家對手方 | **平台**（每筆下注的計價來自平台公式）| 其他玩家或 DMM |
| 平台資本曝險 | 高，需準備 LMSR bond，理論最大損失 = `b * ln(2)` per market | 零，平台僅收手續費 |
| 監管面試敘事 | 需解釋 "為何 AMM 不算莊家"——困難 | "我們是台灣股票交易所" 類比即可 |
| 與 Kalshi 對標 | 不一致（Kalshi 用 Order Book）| 一致 |

決定性因素：**法律地位**。AMM 在某些司法管轄區（特別是英國 GC、美國 CFTC）會直接被認定為對賭對手；即使在 Malta / Curaçao 可申請相應牌照，也仍是「博彩牌照」而非「交易所牌照」，等同接受了「我們就是莊家」的監管假設。這與本專案核心定位牴觸。

### 4.2 為何選 D 而非 B（概率對稱腳本）

> 這是 PRD 原始提案，需要明確處理。

法律上，「掛單動作的法律主體 = 訂單簿上該筆 quote 的對手方」。腳本掛單者是平台，平台就是對手方。即使腳本邏輯只是「維持價差」，這項客觀事實不會改變：

- 玩家成交後虧損 → 對手方賺取了那筆價差 → 該對手方是平台 → 平台從玩家虧損中獲利
- 一旦發生民事爭議或集體訴訟，玩家律師指控「平台用機器人佈局誘騙」，平台無法拿出「但我們是中立平台」的辯護

技術上，概率對稱腳本若**不考慮市場真實機率**，會在突發新聞單邊行情時被反向掃光（無限掛對沖單 → 自有資金一夜清空）。若加上「最大持倉限額」「KILL SWITCH」變得安全，也意味著腳本會在最需要流動性時退場——對玩家 UX 而言更糟（突然停盤）。

### 4.3 為何選 D 而非 C（純 Order Book）

冷啟動流動性問題。Kalshi 也仰賴做市商而非純 P2P；Order Book 模型在沒有外部流動性提供者時對新進玩家極不友善（看到空盤會直接離開）。

### 4.4 為何 DMM 在 LM 之前

詳見 SPEC-009 §4.1。DMM 為合約對手，行為可預測、KYC 清楚、wash trading 風險低；LM 開放任何玩家參與，在反作弊系統未成熟前極易被擼羊毛 / 自我成交套利。先 DMM 後 LM 是「先安全再普及」的順序。

## 5. Consequences

### 5.1 Positive

1. **法律地位清晰**：對任何司法管轄區，平台都可一句話定位「P2P 撮合平台」；監管面試、法律意見書、ToS 起草、玩家爭議辯護都不需要「但是…」。
2. **資本曝險為零**：平台不需準備 AMM bond、不需擔憂單邊行情虧損、不需設計風控對沖。
3. **與 Kalshi 對標**：團隊找參考可直接看 Kalshi 公開技術部落格；招募有 IEX / Coinbase / Kalshi 經驗的撮合工程師時對話成本低。
4. **可審計性**：每筆訂單可追溯到一個自然人或 DMM 對手方。WORM ledger（SPEC-002 §10）可向監管證明「平台從未自己下單」。
5. **長期擴展**：DMM 處理大市場，LM 處理長尾市場，能覆蓋成熟交易所 + 社群預測平台兩種需求。

### 5.2 Negative

1. **冷啟動流動性需要時間**：MVP 上線前必須**完成至少一家 DMM 的合約簽訂與沙盒對接**，否則上線即空盤。
2. **DMM 招募是業務工作，非工程工作**：需要創辦人 / BD 投入 1–3 個月與專業做市商談合約，是不可平行化的關鍵路徑。
3. **DMM API 增加開發 scope**：撮合引擎需額外暴露 binary protocol 給 DMM（SPEC-009 §3.4），約增加 3–4 人週工程量。
4. **maker rebate 是平台短期成本**：DMM 履約期間會吃掉部分手續費收入，預估 MVP 階段平台手續費淨利率被壓低 30–50%。
5. **Liquidity Mining 啟用前置條件嚴格**（SPEC-009 §4.1）：意味著 Phase 1 完全依賴 DMM；若 DMM 招募不順，將拖延 LM 啟用而非倒退使用平台造市。
6. **無法用「演算法保證」永遠有報價**：LMSR 的數學特性是「永遠有報價、永遠可成交」；Order Book 在 DMM 撤單空檔可能短暫沒有報價。

### 5.3 Mitigation Plan

針對 Negative #1 + #2：

- 創辦人與 BD 在 Tech Service Co. 立案的同月開始 DMM 招募管道
- 至少 2 家 DMM 沙盒對接完成才視為 MVP 上線就緒
- 引入第三方做市商評估顧問（可考慮量化交易圈內人脈或前 Kalshi LMM 從業者）

針對 Negative #4：

- DMM rebate 設定有 sunset：第 6 個月起逐月降低 0.025%（從 -0.10% → 第 12 個月 -0.05%）
- 同步推進 Phase 2 LM，分散流動性來源

針對 Negative #6：

- 訂單簿 UI 在「無報價」狀態顯著提示「目前無做市商在線，您可掛限價單」
- 客服腳本準備「為何剛才看不到對手價」標準回應

## 6. Compliance & Legal Boundary

本決策強制以下技術紅線（已寫入 SPEC-009 §1 與 CLAUDE.md）：

```
任何 commit 若包含以下類型程式碼，CI 必須 fail，code review 必須 reject：

- 由「平台帳戶」、「系統帳戶」、「平台公告錢包」自動產生的 NewOrder 訊息
- 無玩家 user_id 對應、由系統內部產生的訂單
- 「自動造市」「對沖機器人」「概率對稱」「market_balancer」
  「auto_quote」「house_book」「probability_symmetry」「auto_hedge」等字眼
  之類別 / 套件 / 函式名
- 在訂單簿 ingressCh 中接受 user_id 為內部 service account 的訂單
```

> **無例外（v1.0.2 起）**：本紅線**無任何 carve-out**。原 v1.0.0 / v1.0.1 之「唯一例外：SPEC-009 §5『冷啟動補貼』極端情境」已於 2026-05-07 撤除（呼應業主決議與 [SPEC-000 §5 R-2](../../../specs/SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker)）。
>
> 平台**永遠不**在自家訂單簿掛單；不存在任何 feature flag、簽核流程、UI 標記、月度揭露機制可以啟用平台 order。流動性不足時，補貼預算改注入 [SPEC-009 §3.3](../../../specs/SPEC-009-dmm-liquidity.md) DMM rebate 放大或市場延後上架，由獨立第三方 DMM 在訂單簿掛單，平台始終維持中立中介角色。
>
> 連動清除清單：
> - [SPEC-009 §5](../../../specs/SPEC-009-dmm-liquidity.md) 整節已移除（保留 placeholder 章節說明移除理由）
> - [SPEC-003 §12.1.1](../../../specs/SPEC-003-matching-engine.md) sentinel `system.platform_announcement` (uid=4) 已移除並列入 `RESERVED_REMOVED_USER_IDS` 防重用集合
> - [SPEC-003 §12.1.4](../../../specs/SPEC-003-matching-engine.md) 「例外情況」內容已改為「**無**」
> - 本 ADR §6 「唯一例外」段落已撤除（v1.0.2）

## 7. Validation Criteria

本決策被視為「成功落地」需滿足：

- [ ] 平台 codebase 通過靜態掃描，確認無 §6 禁止字眼存在
- [ ] DMM 合約模板由 Operating Co. 法律顧問起草完成
- [ ] 至少 2 家 DMM 通過沙盒驗證（SPEC-009 §3.6 第 5 步）
- [ ] MVP 上線首月，主要市場（top 10）皆有 ≥ 1 家 DMM 在線時間 ≥ 90%
- [ ] 監管面試 / 法律意見書中，「平台不持倉」之表述不被質疑
- [ ] Phase 2 啟動前置條件清單（SPEC-009 §4.1）全綠

## 8. References

### 8.1 內部文件

- [SPEC-001 §1.1](../../../specs/SPEC-001-legal-compliance.md) — 合規黃金三原則
- [SPEC-003](../../../specs/SPEC-003-matching-engine.md) — 撮合引擎詳規（Order Book 實作）
- [SPEC-009 §1](../../../specs/SPEC-009-dmm-liquidity.md) — DMM 紅線聲明
- ~~PRD.md §3.2~~ — 原始「概率對稱」提案（已被本 ADR 否決；PRD.md 於 2026-05-05 刪除，git commit `6013b92` 之前可查）

### 8.2 外部參考

- Hanson, Robin (2003). "Combinatorial Information Market Design"（LMSR 原始論文）
- Kalshi Engineering Blog. "How Kalshi's Matching Engine Works"
- IEX Exchange Architecture（Order Book + DPP 設計）
- CFTC Designated Contract Market (DCM) Core Principles

## 9. Notes for Codex / Future Engineers

當你在 SPEC-003 / SPEC-009 對應 task 上動手時：

1. 撮合引擎 ingressCh 必須對 incoming order 強制檢查 `order.user_id != PLATFORM_SYSTEM_USER_IDS`
2. PLATFORM_SYSTEM_USER_IDS 為 sentinel set，**禁止為其授予下單能力**
3. DMM 用獨立 binary protocol 進入引擎，但其訊息進入引擎前同樣轉換為 Order，且 user_id 對應 DMM 法人帳戶（非系統帳戶）
4. 任何「自動下單」想法請先停下，閱讀本 ADR §6 與 §4.2，再決定是否仍要動手
5. 若你認為本 ADR 的決策應該被推翻：**寫一份 ADR-XXXX 標記本 ADR 為 Superseded，附上新的論證與 Compliance 簽核**——不要直接改程式碼

## 10. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-05 | v1.0 | Tech Lead | 首版 Accepted |
| 2026-05-05 | v1.0.1 | Tech Lead | typo-level：§1 與 §8.1 引用之 PRD.md 已於 2026-05-05 隨 SPEC v0.1.1 升級時刪除，將連結改為 git history 回溯說明，避免 dead link。決策內容無變動。 |
| 2026-05-07 | v1.0.2 | Tech Lead | **§6「唯一例外」carve-out 撤除**（業主決議 2026-05-07，呼應 [SPEC-000 §5 R-2](../../../specs/SPEC-000-index.md)）：原 v1.0.0 之「唯一例外：SPEC-009 §5『冷啟動補貼』」段落改寫為「**無例外**」；§6 禁字補 `probability_symmetry` / `auto_hedge` 兩個 v1.0 遺漏字串；連動清除清單涵蓋 SPEC-009 §5 / SPEC-003 §12.1.1 sentinel uid=4 / SPEC-003 §12.1.4。本次屬於 **carve-out 撤除**而非決策反轉（Order Book + DMM 主決策仍 Accepted），故維持 Status: Accepted；嚴格論本次已超出「typo-level」範疇但屬於「強化既有 Compliance 紅線」之 boundary case，紀錄於此供未來 audit。決策內容方向**未反轉**，僅將原本就是「強烈建議永不啟用」的 carve-out 改為「永不可啟用」，與 SPEC-001 §1.1 ② zero tolerance 原則對齊。 |
