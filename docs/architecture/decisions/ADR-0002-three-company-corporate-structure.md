# ADR-0002 — 採用三公司架構（BVI 控股 + 離岸 Operating Co. + 台灣 Tech Service Co.）

| 欄位 | 內容 |
|------|------|
| **Status** | Accepted（**待法律意見書最終背書才能執行**）|
| **Date** | 2026-05-05 |
| **Deciders** | Founder, Tech Lead |
| **Consulted** | 預定：跨境稅務律師、執照地法律顧問、台灣公司法律師、會計師 |
| **Informed** | 工程團隊、財務、HR |
| **Tags** | corporate-structure · legal · tax · regulatory · core-architecture |
| **依賴** | ADR-0001（無業務依賴，但二者共構平台基本面）|

---

## 1. Context and Problem Statement

propheticaNet 在動工之前，必須先決定**法律實體結構**：哪些公司、註冊在哪、彼此關係如何。這個決策會直接影響：

1. **創辦人 / 工程師的刑事責任邊界**（特別是中華民國刑法 §268「圖利供給賭場罪」）
2. **與玩家的合約對手方**（誰跟玩家簽 ToS？玩家儲值錢進誰的帳戶？）
3. **遊戲執照的申請主體**（Malta MGA / Curaçao / PAGCOR 都對申請人之公司類型、最低資本、實質營運地有要求）
4. **員工的雇主歸屬**（特別是台灣工程師如何受雇而不被視為提供賭博服務）
5. **智財權（IP）歸屬**（程式碼、商標、客戶名單）
6. **投資人入股的標的**（投資人投誰？退出時賣誰？）
7. **稅務效率**（台灣 20% 營利事業所得稅 vs 離岸低稅率）
8. **銀行帳戶可開立性**（離岸金流 vs 在地金流）
9. **DR / 商業連續性**（單一 entity 倒閉如何處理）

由於 propheticaNet 之服務形式（即使定位為娛樂遊戲）在台灣司法管轄區仍有被認定為 §268 的風險，**單純設於台灣**會把所有以上風險集中於同一法人，導致創辦人/工程師個人陷入刑事追訴的可能性。

> 必要說明：本 ADR 為**工程與創辦人視角的架構草案**，不是法律意見。最終結構必須由執照地與台灣兩地律師、跨境稅務律師、會計師三方分別出具意見書並簽署後才視為定案。本 ADR 提供論證骨架，供律師補強或調整。

## 2. Decision Drivers

依重要性遞減：

1. **創辦人 / 工程師個人責任隔離**：避免任一名與平台有勞雇關係的台灣居民個人面對台灣 §268 起訴
2. **執照可核發性**：執照地監管要求申請主體須具備一定資本、實質營運、董事資格——必須有適當的 entity
3. **玩家合約主體清晰**：ToS 對手方必須是「持有遊戲執照的離岸法人」，而非台灣公司
4. **IP 與股權保護**：避免單一營運實體出事拖垮 IP 與股權
5. **投資人友善**：未來募資 / 退出時，投資人偏好持有控股公司股票而非 Operating Co. 直接股權
6. **稅務效率**：合法範圍內降低整體稅負
7. **台灣工程人才招募**：必須能在台灣合法雇用工程師且不違反勞動法
8. **銀行帳戶與支付通路可達性**：玩家入金需要有可用的金流通道
9. **維運成本**：複雜架構意味著更高的法務 / 會計 / 行政成本

## 3. Considered Options

### Option A — 全部設在台灣（單一台灣公司）

一家台灣 limited company 同時持有 IP、雇用員工、申請執照（若可能）、與玩家簽約、收受儲值。

### Option B — 全部設在離岸（單一 Malta / Curaçao 公司）

一家離岸 Operating Co. 同時持有 IP、雇用員工（含台灣工程師以離岸雇主身份外派）、執照、玩家對接、金流。台灣不設任何公司。

### Option C — 兩公司架構（離岸 Operating Co. + 台灣 Tech Service Co.）

- 離岸公司：持照、玩家對接、金流、IP
- 台灣公司：純技術 BPO（軟體開發 + 客服外包）

### Option D — 三公司架構（BVI Holdings + 離岸 Operating Co. + 台灣 Tech Service Co.）⭐

```
                    PropheticaNet Holdings (BVI)
                            ↓ 100% 持股
          ┌─────────────────┴─────────────────┐
          ↓                                   ↓
   Operating Co. (Malta/Curaçao)      Tech Service Co. (Taiwan)
   - 持照 / 玩家對接 / 金流              - 軟體研發外包
   - 自有 IP 授權營運                    - 客服外包
                                        - 員工本地僱傭
```

### Option E — 多執照並行（Op Co. 拆兩家：Malta + Curaçao）

D 結構之上，把 Operating Co. 拆成兩家獨立法人，分別持有不同司法管轄區執照，覆蓋更廣的市場。

## 4. Decision Outcome

**選擇 Option D：三公司架構（BVI Holdings + 離岸 Operating Co. + 台灣 Tech Service Co.）。**

> Option E（多執照）是**未來擴展**選項，本階段先以 D 為起點。第 12 個月後可重新評估升級為 E。

### 4.1 為何選 D 而非 A（全部台灣）

| 風險面向 | 全部台灣 | 三公司架構 |
|---------|---------|-----------|
| 創辦人 §268 刑事風險 | **極高** — 全部行為發生於 ROC 境內 | 低 — 玩家對接行為於境外發生 |
| 執照可申請性 | **不可能** — Malta / Curaçao 不對台灣本土公司核發執照（且台灣無對應執照制度）| 可 — Operating Co. 設於執照地 |
| 玩家儲值收受合法性 | **疑慮重大** — 台灣公司收受可能被認定為賭資 | 收款方為離岸法人，符合多數司法區 cross-border 規則 |
| 個資跨境傳輸 | 不適用（皆在台）但個資全集中於台灣 | 玩家個資由離岸 Op Co. 主控，符合執照地監管 |
| 稅務效率 | 統一 20% 營所稅 | 控股層 0%、Op Co. 依執照地（多在 5–15%）、Tech Co. 台灣 20% |

A 的決定性問題：**§268 風險不可控**。即使平台技術上是「無出金的娛樂遊戲」，台灣公司仍可能被檢方認定提供「博弈場所」。一旦起訴，創辦人面對 3 年以下徒刑，工程師可能被列為共犯。**這是不可承擔的個人風險**。

A 否決。

### 4.2 為何選 D 而非 B（全部離岸）

Option B 表面看似最乾淨：法律風險全在離岸、稅率低。但有幾個關鍵問題：

| 議題 | 全部離岸（B）| 三公司（D）|
|------|------------|-----------|
| 台灣工程師雇傭 | 須以離岸雇主身份「外派」，**違反台灣就業服務法**對於本地受僱於外國公司的限制 | Tech Co. 為合法本地雇主 |
| 工程師勞健保 / 退休金 | 需另外掛「人力派遣公司」，灰色 | Tech Co. 直接處理，合法 |
| 台灣徵才品牌 | 「我們是 BVI / Malta 公司」對工程師求職吸引力低 | 「propheticaNet 台灣分部」可正常徵才 |
| 程式碼與技術文件擁有權 | 工程師簽約對象為境外公司，IP 移轉合法性需個案處理 | 員工合約由 Tech Co. 簽，IP 透過 work-for-hire 移轉至 Holdings |
| 銀行對開戶之審查 | 離岸法人在台開帳戶極困難（KYC 嚴格） | Tech Co. 為台灣公司，正常開戶 |
| 監管面試 | 對方可能質疑「沒有實質運營，只是空殼」| 有真實的研發與客服職能可展示 |

B 的決定性問題：**台灣工程師合法雇傭**。即使可以用「個人 contractor」形式繞過，當員工數 > 5 人後就會引起國稅局與勞工局關切，被認定為「實質雇傭關係」要求補繳勞健保與罰款。

B 否決。

### 4.3 為何選 D 而非 C（兩公司架構）

C 已經能處理 4.1 與 4.2 的核心問題。為什麼還要加一層 BVI Holdings？

| 議題 | 兩公司（C）| 三公司（D）|
|------|----------|-----------|
| Op Co. 倒閉風險 | IP 隨之消失 | IP 由 Holdings 持有，可移轉至新 Op Co. |
| 投資人入股 | 直接買 Op Co. 股權 → 投資人持有「博弈牌照公司股權」對機構投資人不友善 | 投資人買 Holdings 股權，Holdings 是中性控股 |
| 多執照擴展（未來轉 E）| 困難 — 第二家 Op Co. 與第一家無共同股東關係 | 容易 — Holdings 直接設立第二家 Op Co. |
| 退出（IPO / M&A）| Op Co. 上市受限於博弈業限制 | Holdings 可上市於對科技公司友善之交易所 |
| 稅務優化 | Op Co. 利潤分潤需直接面對股東稅 | 透過 Holdings 可遞延或合法調節 |
| Operating 與 Holding 信用隔離 | 無 | 玩家爭議或執照糾紛不影響 Holdings 之 IP / 投資人股權 |

C 的核心問題：**Op Co. 是「博弈牌照公司」**，這個身份在投資、銀行、上市場景皆不友善。三公司架構的 Holdings 是「控股科技公司」身份，與 Op Co. 在資產負債表上隔離。

D > C 的差距不在當下運營，而在於**投資人 / 退出 / 監管糾紛時的優勢**。對一個希望募資並長期運營的新創而言，這層差距值得多一家 BVI 公司的維運成本（年費約 USD 1,500–3,000）。

### 4.4 為何 BVI 而非 Cayman / Delaware

控股層選 BVI（British Virgin Islands）的理由：

| 候選 | 優點 | 缺點 |
|------|------|------|
| **BVI** | 設立成本最低 (~USD 1,500/年)、隱私佳、語系英文、亞洲投資人熟悉、無需審計 | 美國 IRS 對 BVI 持有者有額外申報義務（不影響非美投資人）|
| Cayman | 機構投資人最熟悉 | 設立 + 維運成本較高 (~USD 5,000/年)、合規負擔重 |
| Delaware | 美國投資人友善、可上市 | 對亞洲為主之投資人吸引力低、稅務影響大 |
| Singapore | 區域稅務協議多 | 實質營運要求高、不適合純控股 |

決策：**BVI**。後續若 IPO 至納斯達克才考慮重組為 Cayman 或 Delaware。

### 4.5 為何不選 Option E（雙執照並行）

E 本身合理但**首階段成本過高**：

- 兩家 Op Co. 意味著兩套執照申請（Malta + Curaçao 各 USD 50k–200k）
- 兩套合規部門、兩套金流通路、兩套監管報表
- 在尚未驗證商業模式時，重複投入相同基礎建設
- D 已可覆蓋 80% 目標市場（執照地玩家 + 透過 White-label 合作覆蓋部分受限地區）

E 列為**第 12–18 月**升級選項，待 MVP 驗證商業模式且首張執照穩定運作後再評估。

## 5. 三公司職能分工矩陣

> 此節為 SPEC-001 §2.2 之延伸，補充更細的職能對應。

| 職能 | Holdings (BVI) | Operating Co. (執照地) | Tech Service Co. (Taiwan) |
|------|---------------|----------------------|--------------------------|
| 持有股權 | ✅ 唯一持股機構 | ❌ | ❌ |
| 持有 IP（程式碼、商標、域名）| ✅ 主擁有 | 透過授權使用 | 透過授權使用 |
| 持有遊戲執照 | ❌ | ✅ | ❌ |
| 與玩家簽 ToS | ❌ | ✅ | ❌ |
| 收受玩家儲值 | ❌ | ✅（境外信託帳戶）| ❌ |
| 雇用玩家對接客服 | ❌ | 透過 BPO 委外 | ✅（受託執行）|
| 雇用工程師 | ❌ | ❌ | ✅ |
| 撰寫程式碼 | ❌ | ❌ | ✅（IP 透過 work-for-hire 移交 Holdings）|
| 委託開發合約 | Holdings → Op Co. → Tech Co. | 接受 Holdings 授權 | 接受 Op Co. 委託 |
| 結算手續費收入 | ❌ | ✅ | ❌ |
| 支付 BPO 服務費 | ❌ | Op Co. → Tech Co. (B2B 應付) | 接收技術服務費收入 |
| 行銷與品牌 | ❌ | ✅（以 Op Co. 名義）| ❌ |
| 投資人合約 | ✅ | ❌ | ❌ |
| 員工股票期權 | ✅（ESOP 池）| ❌ | ❌（員工持 Holdings ESOP）|

## 6. 金流與服務費流動圖

```
玩家 ─儲值法幣─→ 支付通路 ─→ Op Co. 信託銀行帳戶 (隔離)
                                       │
                                       │ 月度結算
                                       ▼
                              Op Co. 營運帳戶
                                       │
                       ┌───────────────┼───────────────┐
                       │               │               │
                       ▼               ▼               ▼
                  支付通路費       BPO 服務費         營運盈餘
                                  (→ Tech Co.)
                                                       │
                                                       ▼
                                              Holdings 股利分配
                                              (年度)
```

關鍵設計：

- **玩家儲值與營運資金分離**：信託帳戶不可動用於行銷、薪資、稅費（SPEC-002 §2.1）
- **BPO 服務費為市場行情**：Op Co. → Tech Co. 之服務費需符合 arm's length 原則（避免被視為移轉訂價避稅）
- **Tech Co. 不接觸玩家資金**：玩家儲值任何時刻**都不**進入台灣銀行帳戶
- **股利分配遵循執照地稅務協定**：Op Co. → Holdings 之分配適用執照地與 BVI 之雙邊協定

## 7. Consequences

### 7.1 Positive

1. **創辦人 / 工程師個人責任隔離**：在台灣的工程師受雇於 Tech Service Co.，工作內容為「軟體開發」，不接觸玩家資金、不參與市場運營判定，§268 風險可被有效抗辯
2. **IP / 股權與運營風險隔離**：Op Co. 出問題（執照吊銷、玩家集體訴訟）不影響 Holdings 持有的 IP 與股權
3. **投資人友善**：機構投資人可單純買 Holdings 股權，不直接持有「博弈執照公司」股權
4. **退出選項多**：Holdings 結構支援未來 IPO（重組至 Cayman/Delaware）、M&A、Token 化（若監管允許）
5. **執照地監管面試清晰**：Op Co. 是純粹的「執照申請主體」，組織結構簡單可被審查
6. **多市場擴展鋪路**：未來升級至 Option E 時，Holdings 直接設立第二家 Op Co. 即可
7. **稅務合法優化**：透過 BVI Holdings 之控股層，股利分配可被合法地遞延或重新分配
8. **台灣團隊正常徵才**：Tech Co. 是合法台灣公司，工程師可正常受僱、勞健保、職涯敘事正常

### 7.2 Negative

1. **設立成本**：三家公司初期登記費用合計約 USD 8,000–15,000（Holdings BVI ~USD 1,500、Op Co. 依執照地 USD 5,000–10,000、Tech Co. 台灣 ~USD 1,000）
2. **年度維運成本**：合計約 USD 30,000–50,000（執照年費佔大宗）
3. **三套會計與稅務申報**：BVI（簡）、執照地（中）、台灣（中），需要熟悉跨境稅務的會計師事務所
4. **跨境合約複雜**：Holdings → Op Co. 之 IP 授權合約、Op Co. → Tech Co. 之 BPO 服務合約、Tech Co. 員工合約都需專業法律意見
5. **移轉訂價風險**：Op Co. 給 Tech Co. 之服務費若不符合 arm's length，可能被台灣國稅局調整課稅
6. **協調成本**：3 個 entity 之決策需 3 個董事會 / 股東決議；初期創辦人同時擔任三方董事可降低成本
7. **CFC（Controlled Foreign Corporation）風險**：台灣自 2023 年起實施 CFC 制度，台灣居民股東若實質控制離岸 Holdings 與 Op Co.，可能被視為國內所得課稅。需稅務律師個案評估
8. **執照地合規負擔**：Op Co. 在執照地需要實質營運（local director、local office、local AML officer），不可純空殼

### 7.3 Mitigation Plan

針對 Negative #5（移轉訂價）：

- BPO 服務費以「成本 + 合理 markup（建議 7–12%）」計算
- 每年由國際會計師事務所出具 Transfer Pricing Report
- 服務費內容（人月、工作項目）詳細記錄於 BPO 合約附件

針對 Negative #7（CFC）：

- **必須**諮詢專業跨境稅務律師（如 KPMG / PwC / EY 之國際稅務部）
- 評估是否需在 BVI 之上加一層（Holdings 由非台灣居民信託 / 基金會持有）
- 此問題若無法妥善處理可能成為**首要 deal-breaker**

針對 Negative #8（執照地實質營運）：

- Op Co. 需在執照地租用 local office（即使極小，1–2 人 share office 即可）
- 聘請 1 名 local director（可外包，月費約 USD 1,000–2,500）
- 聘請 1 名 local AML / Compliance Officer（可結合 Compliance Officer 角色，月費 USD 3,000–5,000）

針對 Negative #6（協調成本）：

- 初期三個董事會由創辦人 + 1 名共同董事構成
- 制定統一的「集團決策流程文件」，明確哪些決策需哪些董事會通過

## 8. Compliance & Legal Boundary

本決策強制以下不可逾越的紅線：

```
1. 玩家對接行為（簽約、收款、客服回應、市場結算判定、ToS 變更）
   永遠由 Operating Co. 名義執行，禁止以 Tech Service Co. 名義對外
2. Tech Service Co. 銀行帳戶禁止接收任何玩家資金
   （玩家退費走 Op Co. 信託帳戶 → 玩家原入金通道，不經過台灣帳戶）
3. Tech Service Co. 員工禁止：
   - 在執照地以外擁有「結算判定權限」
   - 在台灣境內處理玩家爭議裁決（人工裁決必須由 Op Co. 員工執行）
   - 在台灣境內代表平台對玩家做出任何 binding statement
4. Holdings 帳戶不得進行任何業務交易
   （唯一現金流：Op Co. 股利、新一輪投資款）
5. Op Co. 須在執照地維持實質營運跡象
   （local director、local office、local AML officer 至少各一）
6. 三家公司之間所有資金往來須有 arm's length 合約依據
   （IP 授權費、BPO 服務費、股利分配，皆須合約 + 發票）
```

> 違反以上任一條 → 公司架構保護效力崩潰 → 創辦人 §268 風險回到 Option A 等級。

## 9. 開支預估（首 24 個月，僅供參考）

| 項目 | 一次性 | 年度 |
|------|-------|------|
| BVI Holdings 設立 | USD 1,500 | USD 1,500 |
| Op Co. 設立（Malta 或 Curaçao） | USD 8,000–15,000 | USD 5,000–8,000 |
| Op Co. 執照申請 | USD 50,000–200,000 | USD 25,000–80,000 |
| Op Co. local director / office / AML officer | — | USD 60,000–120,000 |
| Tech Service Co. 設立 | USD 1,000 | USD 800 |
| 三方法律意見書（一次性）| USD 30,000–80,000 | — |
| 跨境稅務顧問 | — | USD 12,000–24,000 |
| 國際會計師事務所 | — | USD 15,000–30,000 |
| Transfer Pricing Report | — | USD 8,000–15,000 |
| **合計（首年）** | **USD 90,500–296,500** | **USD 127,300–279,300** |

> 上述為**法律 / 稅務 / 公司結構**之純開支，不含執照地實質營運薪資、辦公室、員工等。亦不含 Tech Service Co. 在台灣的工程團隊薪資。

## 10. Validation Criteria

本決策被視為「成功落地」需滿足：

- [ ] 三家公司全數完成註冊，提供註冊證書 / Certificate of Incorporation
- [ ] 跨境稅務律師出具意見書，CFC 風險已評估並提供合規路徑
- [ ] Holdings → Op Co. IP 授權合約簽署
- [ ] Op Co. → Tech Co. BPO 服務合約簽署
- [ ] Tech Co. 員工合約模板 + ESOP 規劃完成
- [ ] Op Co. 取得至少一張正式遊戲執照（與 SPEC-001 §10 共驗）
- [ ] Op Co. 在執照地有實質營運跡象（office、director、staff）
- [ ] 三方銀行帳戶開立完成（Holdings 信託 / Op Co. 玩家信託 + 營運 / Tech Co. 營運）
- [ ] 第一年 Transfer Pricing Report 完成
- [ ] 監管面試 / 律師意見書中三公司架構之表述被各方接受

## 11. Open Questions（必須在動工前解決）

1. **Op Co. 執照地最終定案**（影響 Op Co. 設立地）：
   - Malta MGA：聲譽最佳、最嚴、年費 ~EUR 25k、適合走國際品牌路線
   - Curaçao：彈性、一張牌照覆蓋多型態、年費 ~USD 25k、適合 MVP 起步
   - PAGCOR：菲律賓本地門檻高、僅限菲律賓市場、不建議首選
   - Isle of Man：聲譽佳、稅務友善、貴
2. **CFC 風險評估**：台灣居民股東 + 海外控制 → 是否需在 BVI 之上加一層？
3. **Holdings 股權結構**：創辦人直接持有 vs 透過信託 / 基金會持有？
4. **Op Co. 與 Tech Co. 的 BPO markup 比例**：稅務律師建議多少？
5. **是否首階段就引入 Local Director 而非全由創辦人擔任三方董事**？
6. **ESOP 池規模**（Holdings 層級）：10% / 15% / 20%？
7. **未來募資是否將 BVI 重組為 Cayman**？切換時機？

## 12. References

### 12.1 內部文件

- [SPEC-001 §2](../../../specs/SPEC-001-legal-compliance.md) — 經營主體架構（本 ADR 之 SPEC 對應）
- [SPEC-001 §1.1](../../../specs/SPEC-001-legal-compliance.md) — 合規黃金三原則
- [ADR-0001](./ADR-0001-order-book-with-dmm-over-lmsr.md) — 撮合機制（共構平台基本面）
- [SPEC-002 §2.1](../../../specs/SPEC-002-funds-safety.md) — 託管銀行帳戶要求

### 12.2 外部參考

- 中華民國刑法 §266、§268
- 中華民國洗錢防制法
- 台灣《所得稅法》第 43-3 條（CFC 制度）
- Malta Gaming Authority — Type 1/2/3/4 Licence Framework
- Curaçao eGaming — Master / Sub-licence System
- BVI Business Companies Act 2004
- OECD Transfer Pricing Guidelines

## 13. Notes for Codex / Future Engineers

技術實作層面，三公司架構落到 codebase 為以下幾條規則：

1. **資料庫多租戶（multi-tenant）感知**：所有業務資料應隱含 `operating_co_id`，未來支援 Option E（雙執照）時可平滑切分
2. **金流模組分離**：Op Co. 信託帳戶 vs 營運帳戶必須在 ledger 不同 account（SPEC-002 §4.2 已定義）
3. **靜態檔案 / 法律文件**：ToS / Privacy Policy 之文件儲存須帶版本與 issuing entity，記錄為 Op Co. 名義
4. **Email From / Sender**：所有對玩家發信的 sender 須為 Op Co. 之 domain，禁止用 Tech Co. domain
5. **客服系統**：Zendesk / Freshdesk 帳戶歸 Op Co. 名下，Tech Co. 員工以 contractor 身份登入處理
6. **Audit log 中的 operator**：每筆 admin 操作記錄需註明執行者所屬法人（Op Co. employee vs Tech Co. contractor），與 SPEC-001 §8.3 之強制留痕一致
7. **域名 ownership**：主域名（propheticanet.com 之類）由 Holdings 持有，Op Co. 透過 IP 授權使用

## 14. Revision History

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-05 | v1.0 | Tech Lead | 首版 Accepted（待法律意見書最終背書）|
