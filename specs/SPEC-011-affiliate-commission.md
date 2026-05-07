# SPEC-011 — 代理分潤體系

> 優先級：**P2**
> 版本：v0.2.1 · 最後更新：2026-05-07
> 依賴：SPEC-001, SPEC-002, SPEC-005, SPEC-010, SPEC-016
> 被依賴：—

> **v0.2.1 變更（Tech Lead pre-implementation gap closure）**：v0.2.0 為合規層（§2 / §6）成熟版，但落地細節仍有 8 個技術 gap 會導致 Codex 動工後 PR 退回。本版閉環如下，**不**改 §2 / §6 合規骨架：(1) **G-1** §3.2 推薦碼生成釘演算法（BLAKE3 6-byte hex `username_hash` + base32 4-char random，collision retry 5 次後拋 `ERROR_CODE_REFERRAL_CODE_COLLISION`）；(2) **G-2** §3.3 親屬偵測升級為 cross-ref 至 [SPEC-010 §4.2.1](./SPEC-010-security-antifraud.md) `user_link_score` 子規則 `affiliate_kinship_link`，明示偵測特徵 / 觸發時機 / 處置；(3) **G-3** §4.1 達標獎勵改為**延遲 30 天發放**（觀察期內未滿足 5 筆交易 / KYC / 無 chargeback 任一條件即 disqualify，referrer 從未收到 → 不需 clawback、不破 INV-6）；(4) **G-4** §4.2 交易佣金帳本流改為「**獨立每日批次 txn**」，明示 trade transaction 仍維持 SPEC-002 §4.4.3 兩條 entry 不變、commission 為 trade 之**事後**衍生分錄；(5) **G-5** §4.4 chargeback 反沖細化：介紹獎勵走 G-3 延遲發放路徑無需 clawback；持續交易佣金於 referred 被永久封號（SPEC-016 §6.1 chargeback ≥ 1）後**自然停止**，**不**追回封號前已發放佣金（無需 INV-6 carve-out）；(6) **G-6** §4.5 ledger entry 補 sign convention cross-ref 至 [SPEC-002 §4.3](./SPEC-002-funds-safety.md) direction =±1 規則；(7) **G-7** §5.1 「推薦樹結構」用詞改為「推薦清單（flat list）」避免暗示 MLM；(8) **G-8** 新增 §4.6 推薦獎勵上限（每帳號每月 20 個成功介紹獎勵 + 終身 200 個，超過進入 EDD 人工審），原 §10 OQ #1 標 Resolved。對應 [SPEC-000 §5 Q-8](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker)（v0.2.1 新增決策條目）。
> **v0.2.0 變更（R-1 紅線清除）**：依業主決議移除原 §6 「業務帳戶可選擇將點數獎勵轉為 B2B 合約應付」設計，徹底切斷「Player.Wallet → 應付帳款 / 現金」之任何技術路徑。新章節結構：§6.1 業務 B2B 簽約模型（獨立法律關係，非玩家身份升級）/ §6.2 雙身份隔離（業務同時是玩家時的帳本軌跡分離）/ §6.3 帳本層強制執行（INV-6 + DB CONSTRAINT TRIGGER + CI sentinel-scan grep gate）/ §6.4 會計科目分離（`Expense.AffiliateCommission` 玩家點數獎勵 vs `Expense.AffiliatePartnerFee` 業務現金酬勞）。同步修正 §5.2 業務後台「提現申請」誤導描述、§7.1 「升級為業務」誤導描述。對應 [SPEC-000 §5 R-1](./SPEC-000-index.md)。

---

## 1. Objective

設計多層代理推薦體系，但**避免觸及多層次傳銷（MLM）法律紅線**，並確保佣金計算可被稽核、稅務可被追蹤。

## 2. 法律邊界（先擺最前面）

### 2.1 MLM 紅線

依台灣公平交易法 §23 與多層次傳銷管理法，**禁止**：

- 主要收入來自介紹下線而非銷售
- 入會費（推薦人從新人入會費抽佣）
- 三層以上的金字塔結構（嚴格說兩層以上即危險）

### 2.2 安全設計

> 採用「**單層介紹費 + 銷售佣金**」設計，不做傳統意義上的多層代理。

| 設計 | 內涵 |
|------|------|
| **介紹獎勵**（一次性）| 介紹人在被推薦人首次儲值達閾值時獲得固定獎勵點數，僅一層 |
| **交易佣金**（持續）| 介紹人從被推薦人的**交易手續費**中抽取百分比，僅一層 |
| **不存在「下線的下線」** | 推薦關係**只有一層**，不形成樹狀結構 |
| **介紹獎勵以點數發放** | 不是現金，符合「站內虛擬道具」性質 |

> 原 PRD「總代 → 代理 → 業務」的多層架構**不採用**。技術實作上仍可記錄推薦樹用於分析，但**不據此計算佣金**。

### 2.3 「業務」角色（B2B 簽約推廣）

對於希望以「業務」身份大規模推廣的合作方：

- 必須**簽訂正式 B2B 合約**（與 Operating Co.）
- 業務以**個人名義開立發票**（自由業）或**公司對公司結款**
- 業務的「推薦獎勵」歸入**廣告 / 行銷費用**會計科目
- 與一般玩家推薦走**完全分開**的計算與發放路徑

## 3. 推薦關係模型

### 3.1 資料結構

```sql
CREATE TABLE referrals (
    id                          BIGSERIAL PRIMARY KEY,
    referrer_id                 BIGINT NOT NULL,            -- 推薦人
    referred_id                 BIGINT NOT NULL UNIQUE,     -- 被推薦人，每人只能被一人介紹
    referral_code               VARCHAR(32) NOT NULL,
    referred_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_window_ends_at  TIMESTAMPTZ NOT NULL,       -- = referred_at + 30d，§4.1.2 觀察期截止
    qualification_evaluated_at  TIMESTAMPTZ,                -- 觀察期到期 cron 裁決時點
    qualified_at                TIMESTAMPTZ,                -- = qualification_evaluated_at when status='qualified'
    status                      VARCHAR(32) NOT NULL CHECK (status IN (
                                  'pending',           -- 觀察期中
                                  'qualified',         -- 達標已發放
                                  'disqualified',      -- 觀察期未達標
                                  'pending_review',    -- §3.3.1 kinship 待 Compliance MS-4 裁決
                                  'pending_eddreview'  -- §4.6 上限觸發待 EDD 4-eye
                                )),
    signup_bonus_txn_id         BIGINT REFERENCES ledger_transactions(id),  -- 介紹獎勵 ledger txn（§4.5.1）
    metadata                    JSONB NOT NULL DEFAULT '{}'  -- {kinship_flagged, kinship_score, disqualify_reason, eddreview_reason}
);
CREATE INDEX idx_referrals_referrer ON referrals(referrer_id);
CREATE INDEX idx_referrals_pending_eval ON referrals(observation_window_ends_at) WHERE status = 'pending';
CREATE UNIQUE INDEX idx_referrals_code ON referrals(referral_code);
```

### 3.2 推薦碼生成

每位玩家於註冊成功後**同 transaction 內**自動分配唯一推薦碼。

#### 3.2.1 演算法（**G-1 釘死**）

```text
referral_code := lowercase(blake3_6byte_hex(username))
                 + "_"
                 + crockford_base32_random(4)

長度：12 + 1 + 4 = 17 chars
範例：'a3f9c12be801_X9PK'
```

- `blake3_6byte_hex(username)` — BLAKE3 雜湊取前 6 bytes，輸出 12 字元 hex（小寫，不包含 username 原文）
- `crockford_base32_random(4)` — Crockford Base32（去除 I/L/O/U 等易混字元）4 字元亂碼，提供 `32^4 = 1,048,576` 唯一性空間
- **Collision 處理**：
  1. 註冊 transaction 內檢查 `referrals.referral_code` UNIQUE 衝突
  2. 衝突 → 重新抽 `crockford_base32_random(4)`，最多 retry **5 次**
  3. 5 次仍衝突 → 拋 `ERROR_CODE_REFERRAL_CODE_COLLISION`（errors.proto 待 SPEC-014 §6 加入），註冊整體 rollback，玩家收到 `please retry` 提示
  4. 衝突率告警：`proph_referral_code_collision_total` counter，若 > 1 / 1000 註冊持續 1hr → P2 告警（演算法或 username 池異常）
- **不公開**玩家原 username（BLAKE3 為 cryptographic hash，無法反推）
- URL 形式：`https://proph.net/?ref=a3f9c12be801_X9PK`
- 註冊頁自動填入 `referral_code`，僅信任**第一次出現**之 ref param（呼應 §3.3 first-touch attribution）

### 3.3 推薦關係建立規則

- 每位玩家**最多被一人**介紹（first-touch attribution）
- 推薦關係於玩家**註冊時**鎖定，不可後續更改
- 推薦碼**不可由親屬 / 同連結帳號之間使用** — 由 SPEC-010 §4.2.1 `user_link_score` 之子規則 `affiliate_kinship_link` 偵測，**G-2** 細化如下：

#### 3.3.1 親屬偵測子規則 `affiliate_kinship_link`（G-2）

| 偵測特徵 | 觸發權重 |
|----------|---------|
| `referrer.kyc.id_address.country_code + admin_division_l1` 與 `referred.kyc.id_address.*` 完全相同 | +30 |
| `referrer.kyc.last_name`（中文 / 拼音 normalize）與 `referred.kyc.last_name` 相同 | +20 |
| `referrer.kyc.id_address.full_address` Levenshtein distance ≤ 5 | +25 |
| `referrer` 與 `referred` 過去 30 天 IP 重疊 ≥ 1 次 | +20（呼應 SPEC-010 §4.2.1）|
| `referrer` 與 `referred` 共用 device fingerprint | +30（呼應 SPEC-010 §4.2.1）|
| `referrer.kyc.bank_account_holder_name` 與 `referred.kyc.bank_account_holder_name` 相同 | +40 |

執行時機（**雙重 gate**）：

1. **註冊前 gate**：`referred` 完成 KYC Tier 1 時即計算 score。`score ≥ 50` → 註冊允許但**標記** `referrals.metadata.kinship_flagged = true`、推薦關係 `status = 'pending_review'`，介紹獎勵發放路徑暫停。
2. **發放前 gate**（呼應 §4.1 達標檢查）：30 天觀察期到期時若仍 flagged，由 Compliance 4-eye review（呼應 [SPEC-010 §9.3 MS-4](./SPEC-010-security-antifraud.md)）裁決：
   - 通過 → `status = 'qualified'`，發放介紹獎勵
   - 不通過 → `status = 'disqualified'`，referrer 不得獎勵但 referred 註冊維持有效

> 親屬之間若為**業務 B2B 簽約**（§6）關係，由業主與 Compliance 評估個案；不適用本規則自動 disqualify（業務酬勞為現金 KPI，非點數獎勵，紅線在不同處）。

## 4. 佣金計算

### 4.1 介紹獎勵（一次性，**延遲 30 天發放，G-3**）

#### 4.1.1 達標條件（觀察期內須**全部**滿足）

被推薦人於註冊後 30 天觀察期內須累計滿足：

- 首次儲值 ≥ TWD 500
- 完成 KYC Tier 1
- 觀察期內成交**至少 5 筆**訂單（防只儲值不交易）
- 觀察期內**無 chargeback**（呼應 SPEC-016 §6.1，chargeback ≥ 1 觸發永久封號則直接 disqualify）
- 觀察期結束時無 §3.3.1 親屬偵測 `pending_review` 未裁決狀態
- 觀察期結束時 referrer 帳號自身正常（未凍結 / 未封號）

#### 4.1.2 獎勵時點（**delayed-issue 模型**）

> **設計選擇**：採延遲發放而非「達標即發放 + 失敗 clawback」，理由：
> - clawback 需從 referrer wallet 反向出口，但 INV-6（[SPEC-002 §5](./SPEC-002-funds-safety.md)）outflow 白名單**禁止** `Player.Wallet.* → Expense.AffiliateCommission` 之反向流，且玩家可能已花掉點數導致負餘額 + 凍結；技術上代價極高
> - 延遲發放等同於「觀察期內 referrer 從未收到 → 失敗時無物可追回」，從根源繞開 INV-6 衝突，**不**需要新增 INV-6 carve-out
> - 對 referrer 影響小（30 天延遲 vs 立即發放）；對 sock-puppet / chargeback 套利者則完全堵死

執行流程：

```text
T+0  : referred 註冊，referrals.status = 'pending'，無帳本動作
T+0  : referred 完成 KYC Tier 1 + 首次儲值 → metadata 記錄達標時點，但**不發獎勵**
T+0~30: 觀察期累計交易筆數、chargeback 偵測、§3.3.1 kinship review
T+30 : 排程 cron `affiliate-qualification-evaluator`（每日 03:00 台北時間）批次裁決
        ├─ 全部達標   → status = 'qualified'，寫入 affiliate-commission txn
        ├─ 任一未滿足 → status = 'disqualified'，metadata.disqualify_reason 記錄
        └─ kinship pending → 跳過本批次，留待 Compliance 裁決
```

達標後**單次**發放：

```text
推薦人獲得：固定 100 點數（內部成本約 TWD 100）
被推薦人獲得：50 點數註冊獎金（同 txn 內發放，**亦**延後至 T+30 一併發放，避免 referred 達標後 chargeback 套利）
```

> 預算邏輯：行銷成本 ≈ TWD 150 / acquired user，視市場 CAC 調整

### 4.2 交易佣金（持續，**獨立每日批次 txn，G-4**）

被推薦人每筆成交產生的**手續費**中：

```
maker_fee × 10% → referrer
taker_fee × 10% → referrer
```

#### 4.2.1 帳本流（與 trade transaction **嚴格分離**）

> **G-4 釘死**：commission **不**內嵌在 trade entry，避免將撮合 hot path 的 entry 數量翻倍、亦避免 trade 失敗回滾時錯誤回滾 commission。SPEC-002 §4.4.3 之 trade entry 兩條（A/B 各扣 fee → Revenue.TradingFee）**完全不變**。

執行流程：

```text
T+0  : 撮合成交 → SPEC-002 §4.4.3 trade entry 寫入（fee 全額入 Revenue.TradingFee）
T+0  : commission-accrual-worker 訂閱 trade.events Kafka topic
T+0  : 對於每筆 trade，若 trade.user_id 屬於有 active 推薦關係之 referred（未過 12 月 / referrer 帳號正常）
       → 寫入 commission_accruals 表（per-trade staging，**非帳本**）：
       (referrer_id, referred_id, trade_id, fee_amount, commission_amount, accrued_at)
T+1  : 排程 cron `affiliate-commission-payout`（每日 03:30 台北時間）
       ├─ 對每位 referrer 加總前 24h commission_accruals
       ├─ 寫入單一 affiliate_commission txn（idempotency_key='affiliate-commission-{referrer_id}-{date}'）
       └─ 標記對應 commission_accruals 為 paid_out
```

#### 4.2.2 Staging 表

```sql
CREATE TABLE commission_accruals (
    id              BIGSERIAL PRIMARY KEY,
    referrer_id     BIGINT NOT NULL,
    referred_id     BIGINT NOT NULL,
    trade_id        BIGINT NOT NULL UNIQUE,           -- 防 trade 重複計提
    fee_amount      BIGINT NOT NULL CHECK (fee_amount > 0),
    commission_amount BIGINT NOT NULL CHECK (commission_amount > 0),
    accrued_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    paid_out_txn_id BIGINT REFERENCES ledger_transactions(id),
    paid_out_at     TIMESTAMPTZ
);
CREATE INDEX idx_commission_pending ON commission_accruals(referrer_id) WHERE paid_out_txn_id IS NULL;
```

> Staging 表為 application-level 暫存，**不**屬於 ledger（不受 INV-1 雙簿記約束）。actual ledger 寫入發生於每日 batch payout txn。

### 4.3 佣金期限

交易佣金**僅持續被推薦人註冊後 12 個月**，之後不再計算。

> 設計目的：防止「躺賺」，鼓勵推薦人持續推廣新人

### 4.4 反作弊規則

- referrer 與 referred 為同 IP / 同 device fingerprint / 同 KYC 證件 → 由 §3.3.1 `affiliate_kinship_link` 偵測標記 `pending_review`，30 天觀察期到期未通過裁決則 `disqualified`，**從未發放**（呼應 G-2）
- referred 沒有真實交易（僅儲值不交易）→ §4.1.1 「成交至少 5 筆」未達 → `disqualified`，**從未發放**
- referrer 帳戶被標記異常（凍結 / 封號 / EDD pending） → 觀察期到期時若仍異常 → 介紹獎勵跳過本帳號永不發放（不留 pending dangling），交易佣金亦自停止
- **referred 退款 / chargeback**（**G-5 細化**，呼應 [SPEC-016 §6.1](./SPEC-016-payment-gateway.md) chargeback ≥ 1 永久封號）：
  - **介紹獎勵**：依 §4.1.2 延遲發放模型，觀察期內 referred chargeback → `disqualified`，referrer 從未收到 → **不需 clawback、不破 INV-6**
  - **交易佣金（已過觀察期之 referred）**：referred 一旦 chargeback ≥ 1 → SPEC-016 §6.1 永久封號 → 後續無交易 → 持續佣金**自然停止**
  - **chargeback 之前已支付給 referrer 的交易佣金**：**不追回**。理由：(a) referrer 為善意第三方，無從預知 referred chargeback；(b) 追回會破 INV-6 outflow 白名單，需新增 carve-out 反 R-1 紅線設計意圖；(c) 單筆 referred 之累計 commission 通常 ≪ 100 TWD，業務上可吸收為 marketing acquisition cost。本決策對應 SPEC-002 v0.1.8 INV-6 chargeback carve-out **不**擴及 affiliate；wallet-ledger-service migration 之 trigger 維持原 SPEC-002 §5 INV-6 名單即可

### 4.5 帳本記錄

呼應 [SPEC-002 §4.3](./SPEC-002-funds-safety.md) direction 規則（**G-6** 補 sign convention 說明）：

- `direction = +1` ≡ 借方（debit）：對 Expense / Asset / Player.Locked 表示**金額增加**；對 Liability / Revenue 表示**減少**
- `direction = -1` ≡ 貸方（credit）：對 Expense / Asset / Player.Locked 表示減少；對 Liability / Revenue 表示**增加**

`Player.Wallet.*` 為 liability 類別 → `direction = -1` 表示玩家錢包點數**增加**（平台對玩家負債增加）。

#### 4.5.1 介紹獎勵（一次性，§4.1.2 達標 batch 觸發）

```text
Txn: affiliate_signup_bonus, idempotency_key='affiliate-signup-{referral_id}'
metadata: {referrer_id, referred_id, qualified_at, observation_window: '30d'}

  Entry 1: Expense.AffiliateCommission   +100   (direction=+1, 平台費用增加)
  Entry 2: Player.Wallet.{referrer}      -100   (direction=-1, referrer 點數增加)
  Entry 3: Expense.AffiliateCommission   +50    (direction=+1, 註冊獎金亦記入)
  Entry 4: Player.Wallet.{referred}      -50    (direction=-1, referred 點數增加)

  借貸代數和：100 + (-100) + 50 + (-50) = 0 ✓
```

#### 4.5.2 交易佣金（持續，§4.2.1 每日 batch）

```text
Txn: affiliate_commission, idempotency_key='affiliate-commission-{referrer_id}-{date}'
metadata: {referrer_id, period_start, period_end, accrual_count, total_commission}

  Entry 1: Expense.AffiliateCommission   +X     (direction=+1, X = Σ 24h commission_accruals)
  Entry 2: Player.Wallet.{referrer}      -X     (direction=-1, referrer 點數增加 X)

  借貸代數和：X + (-X) = 0 ✓
```

> 兩種 commission 皆走 `Expense.AffiliateCommission → Player.Wallet.{referrer}` 路徑，目的地為 `Player.Wallet.*`（liability），不觸發 INV-6 outflow 白名單檢查（INV-6 規範 outflow，本路徑為 inflow）。

### 4.6 推薦獎勵上限（**G-8 sock-puppet 防護**）

| 限制 | 數值 | 觸發處置 |
|------|------|---------|
| 單一 referrer 月度成功介紹獎勵上限 | 20 個 / 月（曆月）| 超過部分自動進 `pending_eddreview`，由 [SPEC-010 §9.3 MS-N](./SPEC-010-security-antifraud.md) 4-eye review 裁決 |
| 單一 referrer 終身成功介紹獎勵上限 | 200 個（生涯累計）| 達上限後新介紹獎勵全數進 `pending_eddreview`；referrer 系統訊息提醒「達生涯上限，建議轉 §6 業務 B2B 簽約路徑」 |
| 單一 referrer 月度交易佣金上限 | TWD 50,000 等值 / 月 | 超過部分掛 `pending_eddreview`；防 referred 為大戶 wash trading 套佣金 |
| 同 IP / device fingerprint / KYC 證件 referrer 群組月度合計上限 | 同上（合併計算）| 防一人多帳號規避上限 |

> 上限值為 v0.2.1 之初始 default，可由業主於 Beta 後依實際 CAC / LTV 曲線調整；變更須 SPEC-011 patch 並走 [SPEC-010 §9.3 MS-N](./SPEC-010-security-antifraud.md) 簽核（影響行銷預算 P&L）。
> 上限觸發後**不**追回已發放獎勵；僅暫停**新增**獎勵入帳，避免破 INV-6。

## 5. 推薦人後台

### 5.1 自助介面

每位玩家都可在個人頁查看：

- 已成功介紹人數（`status = 'qualified'` 之 referrals 計數）
- 累計獲得獎勵（介紹獎勵 + 交易佣金分項顯示）
- 過去 30 天 / 12 個月歷史（時間序列圖表）
- **推薦清單（flat list，僅一層）**：你介紹的人 — 列出每位 referred 之 username（僅顯示前 3 字 + 後 1 字、其餘以 `*` 遮蔽，避免 PII 揭露）、註冊時點、`status`（`pending` / `qualified` / `disqualified` / `pending_review` / `pending_eddreview`）、累計貢獻佣金（**G-7**：原「推薦樹結構」用詞改為「推薦清單」，避免暗示 MLM 樹狀結構，呼應 §2.2「推薦關係只有一層，不形成樹狀結構」）

### 5.2 業務專屬後台（B2B 簽約）

針對簽約業務的進階介面：

- 細項 dashboard：每位被推薦人的活動、儲值、交易量、佣金
- 月度報表 PDF 下載
- B2B 酬勞月結（業務 invoice + Operating Co. 應付帳款流程，**現金**支付至業務指定銀行帳戶；**完全與玩家錢包點數無關** — 詳見 §6）
- 稅務文件：發票上傳、扣繳憑單下載

> **紅線提醒**：本後台**不**提供任何「玩家錢包點數 → 現金」轉換功能。業務若同時是平台玩家身份，其玩家錢包點數仍遵守 SPEC-001 §1.1 ① 「不可回兌」原則（呼應 §6.2）。

## 6. 業務 / 大型推廣方支付路徑

> **核心原則（R-1 紅線）**：業務 B2B 路徑與玩家點數系統**完全分離**，**永遠不**透過任何技術路徑將「玩家錢包點數」轉換為「法人應付帳款」或「現金」。SPEC-001 §1.1 ① 「點數不可回兌」為 **zero tolerance**，無任何例外（含 B2B 合約包裝、業務身份升級、自願捐贈等）。對應 [SPEC-000 §5 R-1](./SPEC-000-index.md#5-跨檔決策追蹤表cross-cutting-decision-tracker)。

### 6.1 業務簽約模型（獨立法律關係）

業務 B2B 為**全新獨立**之法律關係，**非**「玩家身份升級」：

```
業務（自然人個人 / 公司法人）── 簽訂 B2B 合約 ──→ Operating Co.
                                                    │
                                                    ▼
業務每月依合約 KPI 提交 invoice（介紹合格玩家數 × 單價 + 帶量階梯獎金等）
                                                    │
                                                    ▼
Operating Co. 從**營運帳戶**（非託管帳戶）以**現金**支付至業務指定銀行帳戶
                                                    │
                                                    ▼
業務自行報稅（境外收入或本地收入依其所在國法律）
```

關鍵特徵：

- 業務酬勞**不基於**該業務「玩家錢包」內任何點數餘額
- 業務酬勞**不從**任何玩家錢包出口
- 業務酬勞為獨立 KPI 計算結果，於 §6.4 之 `Expense.AffiliatePartnerFee` 會計科目記錄
- 業務 B2B 合約由 Operating Co. 律師起草，明示「**不含**任何玩家點數轉換條款」

### 6.2 雙身份隔離（業務同時是平台玩家）

若同一自然人**同時**為「業務」與「平台玩家」：

| 維度 | 玩家身份 | 業務身份 |
|------|---------|---------|
| user_id | 例：`user_42` | 例：`partner_007`（獨立 id）|
| 帳本軌跡 | `Player.Wallet.42` 點數 | `Liability.AccountsPayable.partner_007` 應付帳款 |
| 收入性質 | 點數遊戲獎勵（不可回兌）| 現金 KPI 酬勞 |
| 支付通路 | 站內錢包（永不出口）| 銀行 API 匯款（營運帳戶）|
| 稅務 | 玩家獎勵不需報稅（虛擬道具）| 業務酬勞須報稅 |

執行要求：

- 兩個身份在系統內為**完全獨立**之 user_id 與帳本軌跡，**不可**共用同一 wallet
- 玩家身份之**錢包點數**遵守「不可回兌」原則 — 即便該人為簽約業務，**也不能**將其玩家錢包點數轉為現金 / 應付帳款 / 任何形式之兌換
- 後台需強制 KYC 證件對照偵測「業務 partner_id」與「玩家 user_id」之自然人關聯，並於業務 KPI 計算 query 中**排除**「業務本人作為玩家」之自我介紹（防止套自身行為，呼應 §4.4 反作弊）

### 6.3 帳本層強制執行（呼應 SPEC-002 §5 INV-6）

> 為避免任何 application 層 bug、admin UI 誤操作、或日後設計變更將紅線下沉至 ledger invariant；CHECK constraint 比 application check 更強，連 admin 後台都繞不過。

- **新 INV-6**（待 [SPEC-002 v0.1.5](./SPEC-002-funds-safety.md) 落地）：`Player.Wallet.{user_id}` 之 outflow（direction = +1，債務減少）目的地**白名單**僅限：
  - `Player.Locked.{user_id}` — 鎖倉
  - `Player.Frozen.{user_id}` — 凍結（自我排除 / Compliance / EDD）
  - `Revenue.TradingFee` — 交易手續費
  - `Revenue.SettlementFee` — 結算手續費
  - `Player.Wallet.{other_user_id}` — **僅限**結算 trade pair（settle_txn_id 必填）

- **任何**嘗試從 `Player.Wallet.*` 流向以下科目之 ledger entry **必由 PostgreSQL CHECK constraint + DB CONSTRAINT TRIGGER 強制 reject**：
  - `Liability.AccountsPayable.*`（業務應付帳款）
  - `Bank.OperatingAccount.*`（營運帳戶）
  - `Expense.AffiliatePartnerFee`（業務酬勞）
  - 任何銀行 / 支付通路科目
  - 任何外部錢包 / 商品兌換科目

- **CI sentinel-scan**（呼應 [SPEC-017 §8](./SPEC-017-monorepo-bootstrap.md) sentinel-scan job）加 grep gate：

  ```bash
  # R-1 紅線：禁止任何「玩家錢包 → 現金 / 應付」之 code path
  rg -n -e 'Player\.Wallet.*->\s*AccountsPayable' \
     -e 'Player\.Wallet.*->\s*OperatingAccount' \
     -e 'Player\.Wallet.*->\s*Bank\.' \
     -e 'wallet_to_payable|cash_out_player_pts|redeem_points_to_cash' \
     --type rust services/ crates/ \
     && echo "::error::R-1 紅線：禁止 Player.Wallet → 應付/現金 路徑（SPEC-001 §1.1 ①）" && exit 1
  ```

### 6.4 業務酬勞會計科目（與玩家獎勵嚴格分離）

業務 B2B 酬勞與玩家獎勵在帳本層完全分離：

```text
費用（Expenses）
  ├─ Expense.AffiliateCommission       玩家介紹獎勵 / 交易佣金
  │                                    └─→ 對手方為 Player.Wallet.{referrer}
  │                                        （仍是點數，不可回兌；呼應 §4.5）
  │
  └─ Expense.AffiliatePartnerFee       業務 B2B 合約酬勞
                                       └─→ 對手方為 Liability.AccountsPayable.{partner_id}
                                           （現金應付，與玩家錢包零交集；呼應 §6.1）
```

兩者**永遠不**相互轉換、相互抵銷、相互沖銷。月結時各自獨立揭露。SPEC-002 §4.2 Chart of Accounts 將同步補上 `Expense.AffiliatePartnerFee` 與 `Liability.AccountsPayable.*` 子科目。

## 7. 稅務處理

### 7.1 玩家層級

- 玩家獲得的點數獎勵**不是現金所得**，依台灣稅法不需報稅
- 但**B2B 業務**獲得的 B2B 收入須報稅
- 玩家若達一定推廣規模（每年介紹超過 X 人）→ 系統提醒可**另外**簽約 B2B 業務獲得現金 KPI 酬勞；此業務簽約**不會**將玩家現有錢包點數轉為現金（玩家錢包點數永遠不可回兌，呼應 [SPEC-001 §1.1 ①](./SPEC-001-legal-compliance.md)），業務酬勞為**新起算**之 B2B 合約收入（§6.1），與玩家身份的點數歷史完全分離

### 7.2 平台層級

- 介紹獎勵與交易佣金記入 `Expense.AffiliateCommission`
- 月結提供財務 / 會計 / 稅務團隊
- 跨境支付（業務在不同國家）依執照地稅法處理

### 7.3 扣繳義務

- B2B 業務若為台灣居民，依所得稅法扣繳 10%
- 跨境支付依雙邊租稅協定處理
- 系統需自動產生扣繳憑單

## 8. 監管揭露

執照地監管可能要求揭露：

- 推薦獎勵總額（月 / 季）
- B2B 業務名單（執照地律師審閱）
- 大額推薦獎勵個案（單月 > USD 5,000 等值）

## 9. 驗收條件

### 9.1 後端（Codex owner）

- [ ] `referrals` 表 schema 上線（§3.1）並有 `referred_id` UNIQUE constraint 防雙介紹
- [ ] `commission_accruals` staging 表 schema 上線（§4.2.2）並有 `trade_id` UNIQUE constraint 防 trade 重複計提
- [ ] 推薦碼生成函式（BLAKE3 + Crockford Base32 + collision retry，§3.2.1）落地 `crates/sdk/src/referral/code.rs`，含 unit test：collision retry / 衝突上限拋錯 / 字元集合驗證
- [ ] §3.3.1 `affiliate_kinship_link` 子規則納入 SPEC-010 §4.2.1 `user_link_score` 計算
- [ ] cron `affiliate-qualification-evaluator`（每日 03:00 台北）批次裁決 30 天觀察期（§4.1.2）
- [ ] cron `affiliate-commission-payout`（每日 03:30 台北）批次發放交易佣金（§4.2.1）
- [ ] §4.6 上限規則實作（月度 / 終身 / 群組 IP-fingerprint 合併）
- [ ] 介紹獎勵與交易佣金 ledger txn 寫入符合 §4.5.1 / §4.5.2 entry 結構，借貸代數和為 0（INV-1）
- [ ] referrer 達上限 / 觀察期失敗 / kinship pending 之系統通知（站內訊息）

### 9.2 前端（Tech Lead 自主開發）

- [ ] 玩家自助介面 `/account/affiliate` 顯示推薦數據（§5.1，flat list 而非樹狀）
- [ ] 推薦碼顯示與「複製分享連結」UX
- [ ] 公開行銷頁 `/affiliate` 已存在（[`apps/web/app/affiliate/page.tsx`](../apps/web/app/affiliate/page.tsx)）並對齊 §2 / §6 紅線描述

### 9.3 跨 SPEC 依賴

- [ ] SPEC-010 §4.2.1 `user_link_score` 表新增 `affiliate_kinship_link` 子規則
- [ ] SPEC-014 §6 errors.proto 新增 `ERROR_CODE_REFERRAL_CODE_COLLISION` / `ERROR_CODE_REFERRAL_CAP_EXCEEDED`

### 9.4 法律 / 業務 owner

- [ ] B2B 業務簽約模板由律師完成（§6.1）
- [ ] 月結報表自動產生，財務可下載

## 10. Open Questions

1. ~~推薦獎勵點數是否設上限（防 1 人推薦 1000 個 sock puppet）？~~ ✅ Resolved v0.2.1（§4.6 月度 20 / 終身 200 / 月度佣金 TWD 50k）
2. 12 個月佣金期限是否合適？業界多為 lifetime 但風險高（保留待 Beta 後依 LTV / churn 數據再評估）
3. 「業務」升級門檻？月介紹 ≥ X 人 / 累計收入 ≥ X？（合約事宜，待業主與執照地律師決定）
4. 業務 B2B 合約是否要求最低推廣承諾？（合約事宜）
5. 業務違反條款（如假帳號刷量）的清退與已付佣金回收條款？（合約事宜）
6. §4.6 月度 20 / 終身 200 上限若 Beta 後實際 CAC 顯示偏低或偏高，調整流程與 governance？（v0.2.1 default，調整需 SPEC patch + MS-N 簽核）
