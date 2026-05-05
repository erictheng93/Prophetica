# SPEC-015 — 市場生命週期管理（Market Lifecycle）

> 優先級：**P0（必須在第一行 production code 之前完成）**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-001, SPEC-003, SPEC-004
> 被依賴：SPEC-002, SPEC-009, SPEC-010

---

## 1. Objective

定義一個市場（Market）從**創建提案 → 上架審查 → 開放下單 → 關閉 → Oracle 結算 → 終結**的完整生命週期，並建立可被監管稽核的決策軌跡與內容合規邊界。本 SPEC 補齊 SPEC-003（撮合）與 SPEC-004（結算）之間的「誰決定要創哪些市場」議題。

## 2. 範圍與不在範圍

### 2.1 In-scope

- 市場創建提案、審查、上架、撤銷流程
- 市場類別與內容合規邊界
- Oracle 來源指派
- 開盤 / 關盤 / 結算時間定義
- 操作者權限與多人簽核
- 對監管稽核的留痕要求
- 與 SPEC-003 撮合引擎、SPEC-004 結算流水線的銜接點

### 2.2 Out-of-scope（由其他 SPEC 處理）

- 市場內部撮合演算法（SPEC-003）
- 結算狀態機 / 申訴 / 爭議裁決（SPEC-004）
- 帳本記錄（SPEC-002）
- DMM 招攬與報價（SPEC-009）

## 3. 市場狀態機

```
   ┌───────────┐
   │ DRAFT     │  Operator 起草中、未送審
   └─────┬─────┘
         │ 送審
         ▼
   ┌───────────┐
   │ REVIEW    │  Compliance + Legal 雙簽
   └──┬─────┬──┘
      │     │ 拒絕
      │     ▼
      │  ┌──────────┐
      │  │ REJECTED │  終態
      │  └──────────┘
      │ 通過
      ▼
   ┌───────────┐
   │ SCHEDULED │  審查通過、等待 OpensAt
   └─────┬─────┘
         │ market.OpensAt 到達
         ▼
   ┌───────────┐
   │ OPEN      │  進入 SPEC-003 撮合（接受下單與撤單）
   └─────┬─────┘
         │ market.ClosesAt 到達
         ▼
   ┌───────────┐
   │ CLOSED    │  進入 SPEC-004 結算流水線
   └─────┬─────┘
         │ Oracle 提交 → PENDING → SETTLING → SETTLED / CANCELLED
         ▼
   ┌───────────────────┐
   │ SETTLED / CANCELLED │  終態（由 SPEC-004 §2 接管）
   └───────────────────┘
```

> 本 SPEC 控管 `DRAFT → REVIEW → SCHEDULED → OPEN → CLOSED`；`CLOSED` 之後交給 SPEC-004。

## 4. 角色與權限

| 角色 | 職責 | 可執行操作 |
|------|------|-----------|
| **Market Operator** | 提案市場 | DRAFT 狀態增 / 改 / 送審 |
| **Compliance Reviewer** | 合規審查 | REVIEW 狀態接受 / 拒絕（與 Legal 共同 2-of-2）|
| **Legal Reviewer** | 法律審查 | REVIEW 狀態接受 / 拒絕（與 Compliance 共同 2-of-2）|
| **Schedule Manager** | 開盤排程 | 可調整 OpensAt / ClosesAt（SCHEDULED 狀態，需 4-eye）|
| **Compliance Officer** | 應急處置 | 任何狀態強制下架（記錄理由）|

> 同一名員工**不可**同時擔任 Operator 與 Reviewer。

## 5. 市場分類與內容邊界

### 5.1 允許的市場類別

| 類別 | 範例 | 風險 | 必要 Oracle |
|------|------|------|------------|
| **政治選舉** | 「2028 美國總統大選誰勝出」| 中 | 選舉委員會官網 |
| **體育賽事** | 「世界盃決賽 A 隊 / B 隊勝」| 低 | 主辦聯盟官網 + 二手媒體交叉 |
| **金融指標** | 「年底 BTC > $100k」| 中 | 三家獨立交易所中位 |
| **天氣 / 氣候** | 「明年 7 月台北月均溫 > 30°C」| 低 | 中央氣象署官網 |
| **企業事件** | 「Apple Q3 EPS 是否 > $1.5」| 中 | 公司財報 |
| **流行文化** | 「奧斯卡最佳影片是某電影」| 低 | 頒獎機構公告 |

### 5.2 永久禁止的市場類別（紅線）

> 違反任一條視同 SPEC-001 §1.1 三原則破口，立即下架 + Compliance 記錄。

- ❌ **個人傷亡 / 健康事件**（如「某政治人物在某日前是否過世」）— 不論意圖，市場存在本身即構成倫理風險與激勵殺人 / 自殺
- ❌ **特定個人犯罪 / 入獄 / 起訴結果**（如「某人是否被起訴」）— 司法程序博弈，多數司法區明文禁止
- ❌ **未公開的內幕資訊**（如「某公司是否將被併購」於宣告前）— 內幕交易誘因
- ❌ **戰爭爆發 / 大規模傷亡時間點**（如「某地某月戰事爆發」）— 倫理紅線
- ❌ **個資 / 隱私事件**（如「某名人是否離婚」）— 個資法 / 名譽侵害風險
- ❌ **本平台 / 創辦人 / 員工 / 母公司之 KPI**（如「prophetica DAU 是否破萬」）— 利益衝突
- ❌ **可被個人意志單方面影響之事件**（如「某網紅明天是否上線」）— 操縱風險
- ❌ **未成年涉事題材**（如未成年運動員、未成年公眾人物）

### 5.3 灰色地帶處理

當市場提案落在「未明確禁止但風險不明」時：

1. Compliance Reviewer 要求 Legal Reviewer 出具書面意見
2. 若兩位 Reviewer 仍意見分歧 → 升級至 Compliance Officer 拍板
3. 拍板紀錄寫入 `market_reviews` 表，作為未來判例

## 6. 市場資料模型

```sql
CREATE TABLE markets (
    id                  VARCHAR(64) PRIMARY KEY,           -- 'mkt_2028_us_election'
    title               VARCHAR(255) NOT NULL,
    description         TEXT NOT NULL,
    category            VARCHAR(64) NOT NULL,              -- politics/sports/finance/...
    status              VARCHAR(32) NOT NULL,              -- draft/review/scheduled/open/closed/settled/cancelled/rejected
    operator_id         BIGINT NOT NULL,
    compliance_reviewer_id BIGINT,
    legal_reviewer_id   BIGINT,

    -- 時間
    opens_at            TIMESTAMPTZ NOT NULL,
    closes_at           TIMESTAMPTZ NOT NULL,
    settles_at          TIMESTAMPTZ NOT NULL,

    -- Oracle
    oracle_type         VARCHAR(32) NOT NULL,              -- official_announcement/media/api/manual
    oracle_source_url   TEXT,                              -- 官方來源 URL（可多筆，用 JSON）
    oracle_resolution_rules TEXT NOT NULL,                 -- 結算判定規則（白話文）

    -- 撮合參數（→ SPEC-003 Market struct）
    min_tick_pts        INT NOT NULL DEFAULT 1,
    min_lot             INT NOT NULL DEFAULT 1,
    max_order_qty       BIGINT NOT NULL,
    max_position_per_user BIGINT NOT NULL,
    fee_bps             INT NOT NULL DEFAULT 200,

    -- 稽核
    risk_assessment_url TEXT,                              -- 上架前風險評估文件
    operating_co_id     VARCHAR(32) NOT NULL,              -- 由哪一家 Op Co. 持有（呼應 ADR-0002）

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at        TIMESTAMPTZ,
    approved_at         TIMESTAMPTZ,
    rejected_at         TIMESTAMPTZ,
    rejection_reason    TEXT,
    cancelled_at        TIMESTAMPTZ,
    cancellation_reason TEXT
);
CREATE INDEX idx_markets_status ON markets(status);
CREATE INDEX idx_markets_opens_at ON markets(opens_at);

CREATE TABLE market_reviews (
    id              BIGSERIAL PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL REFERENCES markets(id),
    reviewer_id     BIGINT NOT NULL,
    reviewer_role   VARCHAR(32) NOT NULL,                  -- compliance/legal
    decision        VARCHAR(32) NOT NULL,                  -- approve/reject/require_changes
    notes           TEXT,
    evidence_urls   JSONB,                                 -- 引用的外部證據連結
    decided_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE market_audit_logs (
    id              BIGSERIAL PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL,
    operator_id     BIGINT NOT NULL,
    action          VARCHAR(64) NOT NULL,                  -- create/edit/submit/approve/reject/schedule/cancel
    before_data     JSONB,
    after_data      JSONB,
    ip              INET,
    user_agent      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- §6.2 結果腿（outcomes）
-- 二元市場每筆 markets 對應兩列（YES / NO）。設計成獨立表而非 hardcode，
-- 是為了未來 N 元市場擴展不必 schema migration。
CREATE TABLE market_outcomes (
    id              VARCHAR(80) PRIMARY KEY,               -- 'mkt_X:YES' / 'mkt_X:NO'
    market_id       VARCHAR(64) NOT NULL REFERENCES markets(id) ON DELETE RESTRICT,
    outcome         VARCHAR(8) NOT NULL,                   -- 'YES' / 'NO' / (未來) 多元值
    label           TEXT NOT NULL,                         -- UI 顯示用，如「主隊勝」
    sort_order      SMALLINT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (market_id, outcome)
);
CREATE INDEX idx_market_outcomes_market ON market_outcomes(market_id);

-- §6.3 Oracle 提交（SPEC-004 §4.2 schema 集中於此）
-- 兩位 Operator 獨立寫入；market-service 比對 outcome_result 一致才推進至 PENDING。
CREATE TABLE oracle_submissions (
    id                  VARCHAR(64) PRIMARY KEY,
    market_id           VARCHAR(64) NOT NULL REFERENCES markets(id),
    outcome_result      VARCHAR(8) NOT NULL,               -- 'YES' / 'NO' / 'VOID'
    submitted_by        BIGINT NOT NULL,
    submitted_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence_urls       JSONB NOT NULL,                    -- ['https://...', ...]
    evidence_hash       CHAR(64) NOT NULL,                 -- R2 manifest SHA256
    notes               TEXT,
    signature           BYTEA NOT NULL,                    -- operator 數位簽章
    UNIQUE (market_id, submitted_by)                       -- 同 operator 同市場僅一筆
);
CREATE INDEX idx_oracle_submissions_market ON oracle_submissions(market_id);

-- §6.4 申訴（SPEC-004 §5）
CREATE TABLE disputes (
    id                  VARCHAR(64) PRIMARY KEY,
    market_id           VARCHAR(64) NOT NULL REFERENCES markets(id),
    user_id             BIGINT NOT NULL,
    reason              TEXT NOT NULL,
    evidence_urls       JSONB NOT NULL,
    claimed_outcome     VARCHAR(8) NOT NULL,
    status              VARCHAR(32) NOT NULL DEFAULT 'pending_review',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (market_id, user_id)                            -- 同玩家同市場僅一次申訴
);
CREATE INDEX idx_disputes_market_status ON disputes(market_id, status);

-- §6.5 爭議裁決（SPEC-004 §6.3 schema 集中於此）
-- 3 名 Adjudicator 合議制；votes JSONB 內含 [{user_id, vote, reason, ts}]。
CREATE TABLE dispute_resolutions (
    id                  VARCHAR(64) PRIMARY KEY,
    market_id           VARCHAR(64) NOT NULL REFERENCES markets(id),
    decision            VARCHAR(32) NOT NULL,              -- uphold_initial/overturn/void_market/extend
    votes               JSONB NOT NULL,                    -- [{user_id, vote, reason, ts}, ...]
    voted_at            TIMESTAMPTZ NOT NULL,
    final_signoff_by    BIGINT,                            -- Compliance Officer
    final_signoff_at    TIMESTAMPTZ,
    audit_hash          CHAR(64) NOT NULL                  -- 內容 hash，不可竄改稽核
);
CREATE INDEX idx_dispute_resolutions_market ON dispute_resolutions(market_id);

-- §6.6 玩家持倉（per market_outcome）
-- 一個 trade pair 產生兩筆 positions（買 YES 一筆、買 NO 一筆）。
-- locked_amount 對應該 position 在 escrow 中的鎖倉（內部單位 × 100）。
-- 結算時逐 position 處理（呼應 SPEC-002 §4.4.4 NOTE 1）。
CREATE TABLE positions (
    id                  VARCHAR(64) PRIMARY KEY,           -- ULID
    user_id             BIGINT NOT NULL,
    market_id           VARCHAR(64) NOT NULL REFERENCES markets(id),
    outcome             VARCHAR(8) NOT NULL,
    qty                 BIGINT NOT NULL CHECK (qty > 0),
    avg_price_pts       INT NOT NULL CHECK (avg_price_pts BETWEEN 1 AND 99),
    locked_amount       BIGINT NOT NULL CHECK (locked_amount > 0),
    opened_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at           TIMESTAMPTZ,                       -- 結算或對沖平倉後
    settle_txn_id       BIGINT,                            -- 結算對應 ledger_transactions.id
    metadata            JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX idx_positions_user ON positions(user_id, market_id);
CREATE INDEX idx_positions_market_outcome ON positions(market_id, outcome) WHERE closed_at IS NULL;
```

### 6.7 表間關係簡圖

```
markets ──┬──< market_outcomes
          ├──< market_reviews
          ├──< market_audit_logs
          ├──< oracle_submissions
          ├──< disputes
          ├──< dispute_resolutions
          └──< positions ──> ledger_transactions (settle)
```

> **Schema 真相來源規則**：
> - 上述 6 張表為 SPEC-015 唯一 schema 定義來源。
> - SPEC-004 §6.3 之 `dispute_resolutions` 表結構**已遷移至本 SPEC**，SPEC-004 將以「呼應 SPEC-015 §6.5」方式引用。
> - 任何欄位變更必須更新 `packages/proto/v1/market.proto` 對應 message。

## 7. 上架審查流程（DRAFT → REVIEW → SCHEDULED）

### 7.1 提案要求

Operator 在 DRAFT 狀態必須填寫：

| 欄位 | 規則 |
|------|------|
| 標題 | < 80 字、清楚、無歧義 |
| 描述 | < 500 字、含結算判定規則 |
| 分類 | 從 §5.1 列表擇一；不可自創 |
| Oracle 來源 | 至少 1 個官方 URL；非官方來源需 ≥ 3 個獨立來源 |
| 結算判定規則 | 白話文，回答「在什麼情況下 YES 中獎、什麼情況下 NO」 |
| 風險評估文件 | 上傳 PDF 至 R2 桶（Object Lock），列出已知爭議點 |
| 開盤 / 關盤 / 結算時間 | 三個時間順序必須單調遞增；最早關盤距離結算 ≥ 24 hr（呼應 SPEC-004 §5.1 申訴窗口）|

### 7.2 雙簽核要求（2-of-2）

- Compliance Reviewer + Legal Reviewer **必須各自獨立**寫入 `market_reviews` 條目
- 兩者皆 `approve` → 市場進入 SCHEDULED
- 任一 `reject` → 市場退回 DRAFT，Operator 修改後重送
- 任一 `require_changes` → 退回 DRAFT，附改善要點
- 雙方意見矛盾（一 approve 一 reject）超過 24 hr 未化解 → 升級至 Compliance Officer

### 7.3 Reviewer 自我利益衝突排除

| 排除情境 | 規則 |
|---------|------|
| Reviewer 自身或一等親屬持有相關政治職位 | 禁止參與政治選舉市場審查 |
| Reviewer 自身於該領域有商業利益 | 主動申報並請求迴避 |
| Reviewer 與 Operator 為一等親屬或同住家庭 | 強制系統阻擋 |

## 8. 開盤與關盤排程

### 8.1 OpensAt 至 ClosesAt 必須條件

- `closes_at - opens_at ≥ 1 hour`（最低 1 小時開放，避免人為操盤）
- `closes_at < settles_at`（必須留時間給 Oracle）
- `settles_at - closes_at ≥ 24 hours`（呼應 SPEC-004 §5.1 申訴窗口）
- 同一玩家不可在同一市場上同時持 YES + NO（hedge 風險）— 由 SPEC-003 §12 風控前置層強制

### 8.2 自動狀態轉換

```
SCHEDULED → OPEN：cron 每分鐘檢查 opens_at，到達即發 MarketStatusChanged 事件
OPEN → CLOSED：cron 每分鐘檢查 closes_at，到達後：
  - 撮合引擎拒收新訂單（SPEC-003 §14.4）
  - 訂單簿上活躍訂單全部 cancel + 鎖倉 reverse
  - 觸發 SPEC-004 §2 PENDING 流程
```

### 8.3 排程變更限制

- SCHEDULED 狀態：仍可調整時間，需 4-eye（Schedule Manager + Compliance）
- OPEN 狀態：**只可縮短** ClosesAt（最早可截至「現在 + 5 分鐘」），**不可延長**（避免做局）
- CLOSED 狀態：時間欄位 immutable

## 9. 強制下架（極端情境）

### 9.1 觸發條件

- Oracle 來源失效（官方撤回、URL 死亡、來源被認定非權威）
- 市場本身踩到 §5.2 紅線（事後發現）
- 法規環境劇變（執照地監管要求下架）
- 偵測大規模操縱 / 內幕交易 / 結算前異常持倉集中（呼應 SPEC-010 §4.2.3）

### 9.2 下架流程

1. Compliance Officer 提交下架理由 + 證據
2. 簽核：Compliance Officer + Tech Lead + Legal（3-of-3）
3. 系統執行：
   - 市場狀態變 CANCELLED（不論當下處於哪一階段）
   - 撮合引擎停盤（SPEC-008 §9）
   - 持倉依 SPEC-004 §8.2 加權平均成本退款
4. 公告全玩家、退款完成後 7 天內提交監管報告

## 10. 與 SPEC-003 / SPEC-004 的銜接

### 10.1 對 SPEC-003

`markets` 表的撮合相關欄位（`min_tick_pts` / `min_lot` / `max_order_qty` / `max_position_per_user` / `fee_bps`）會在 SCHEDULED → OPEN 時序列化為 SPEC-003 §3.1 `Market` struct，注入撮合引擎 in-memory。

OPEN 後撮合參數**不可變更**（SPEC-003 §17.3 滾動升級不允許改參數）；要變更必須先 CLOSED。

### 10.2 對 SPEC-004

`markets` 進入 CLOSED 後，由 SPEC-004 §2 結算狀態機接手。本 SPEC 不重複定義 PENDING / DISPUTED / SETTLING / SETTLED / CANCELLED 之轉換。

`markets.oracle_source_url` 與 `oracle_resolution_rules` 直接被 SPEC-004 §4 Oracle Operator UI 顯示。

## 11. 監管報表與留痕

### 11.1 月度報表

- 該月新上架市場數（按分類）
- 該月被拒絕的提案數 + 拒絕理由分布
- 該月強制下架數 + 理由分布
- 該月平均 Reviewer 處理時間

### 11.2 全量留痕

- `market_audit_logs` 7 年保留（呼應 [SPEC-001 §1.1 ③ Full Audit Trail](./SPEC-001-legal-compliance.md#11-合規黃金三原則non-negotiable)）
- `market_reviews` 5 年保留
- `markets.risk_assessment_url` 指向 R2 Object Lock，5 年不可竄改

> 各資料類別之具體保留期限與不可竄改性技術配置（Object Lock / hash chain）由業主決定後注入 R2 桶配置，本 SPEC 僅規範技術介面與最低 7 年要求。

## 12. 性能 SLO

| 指標 | 目標 |
|------|------|
| 提案 → 進入 REVIEW | 即時 |
| REVIEW → SCHEDULED 中位處理時間 | < 24 hr |
| SCHEDULED → OPEN 自動切換誤差 | < 60 秒 |
| OPEN → CLOSED 自動切換誤差 | < 5 秒（影響玩家可預期性）|
| 下架告警 → 撮合停盤 | < 30 秒 |

## 13. 安全要求

- Operator / Reviewer 帳號強制 hardware MFA（YubiKey）
- Reviewer 簽核 API 步進 MFA（每次審查重新驗證）
- 所有 API 走內部 VPN 或固定辦公室 IP
- Audit log SIEM 即時轉發

## 14. 驗收條件

- [ ] markets / market_reviews / market_audit_logs 三張表 schema 上線
- [ ] DRAFT → REJECTED / SCHEDULED 流程通過 E2E 測試
- [ ] 2-of-2 簽核強制驗證（單簽 + 嘗試自簽自核必須失敗）
- [ ] §5.2 紅線項目可在 §5.3 灰色地帶決策時被檢索引用
- [ ] OPEN → CLOSED 自動切換誤差 SLO 達標
- [ ] 強制下架流程演練通過 RTO 與通知 SLA
- [ ] 月度報表自動產生並包含 §11.1 所有欄位
- [ ] Reviewer 利益衝突排除規則通過模擬攻擊測試

## 15. Open Questions

> 跨檔決策已收斂至 [SPEC-000 §5](./SPEC-000-index.md) 中央追蹤表。本節僅保留 SPEC-015 自身範圍內議題。

1. 是否引入「市場創建 RFC」公開徵集機制（讓玩家提案）？或保留純 Operator 提案？
2. 市場分類是否要做 i18n 翻譯（首發 5 國語言），影響 §5.1 列表的維護成本
3. 大型賽事（如世界盃）是否一次批准多個市場（決賽結果、進球數、紅牌數），或逐筆審查？
4. 灰色地帶判例（§5.3）是否公開（給玩家可預期）或內部保留？
5. Oracle 來源失效（如官方下線）的自動偵測機制：每日 ping URL 還是事件驅動？
