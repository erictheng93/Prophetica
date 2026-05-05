# SPEC-004 — 結算與爭議處理

> 優先級：**P0**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-002, SPEC-003
> 被依賴：SPEC-007, SPEC-012

---

## 1. Objective

設計從「市場關閉」到「玩家點數變動完成」的完整結算流水線，能處理正常結算、延期、取消、爭議申訴、特殊情境（如平局、結果無法判定），並提供 24 小時申訴窗口、不可竄改的判定紀錄、與多人簽核稽核軌跡。

## 2. 結算狀態機

```
                       ┌────────────┐
                       │   OPEN     │  接受下單與撤單
                       └──────┬─────┘
                              │ market.ClosesAt 到達
                              ▼
                       ┌────────────┐
                       │   CLOSED   │  停止收單，撤銷未成交訂單
                       └──────┬─────┘
                              │ Oracle 提交初步結果
                              ▼
                       ┌────────────┐
                       │  PENDING   │  公告「初判結果」+ 24hr 申訴窗口
                       └──┬─────┬─────┐
                  申訴成立  │     │     │ 無申訴 / 申訴駁回
                          │     │     │
                          │     │     ▼
                          │     │  ┌──────────────┐
                          │     │  │  SETTLING    │  執行帳本結算
                          │     │  └──────┬───────┘
                          │     │         │
                          │     │         ▼
                          │     │  ┌──────────────┐
                          │     │  │  SETTLED     │  完成 (terminal)
                          │     │  └──────────────┘
                          │     │
                          │     ▼
                          │  ┌──────────────┐
                          │  │  DISPUTED    │  人工複審中
                          │  └──┬───────────┘
                          │     │ 結論
                          ├─────┘
                          ▼
                    ┌────────────┐
                    │ CANCELLED  │  原路退回所有持倉 (terminal)
                    └────────────┘
```

## 3. 角色與權限

| 角色 | 職責 | 數量 |
|------|------|------|
| Oracle Operator | 提交初步結果 | 至少 2 名 |
| Settlement Reviewer | 複核結算前 | 至少 1 名 |
| Dispute Adjudicator | 爭議裁決 | 至少 3 名（合議制）|
| Compliance Officer | 監管申報、最終 sign-off | 1 名 |

> 任一單一角色不得獨立完成結算。所有結算狀態變更必須 **2-of-N 簽核**（multi-sig）。

## 4. Oracle 機制

### 4.1 Oracle 來源類型

| 類型 | 適用市場 | 信任機制 |
|------|---------|---------|
| **官方公告** | 政府公告（選舉、統計、財報）| 截圖 + URL + 簽章備存 |
| **權威媒體** | 突發事件、體育賽事 | 三家獨立來源交叉 |
| **API 自動化** | 加密貨幣價格、體育比分 | 外部 API + 簽章 |
| **人工裁定** | 模糊事件、爭議案件 | 合議制，多數決 |

### 4.2 Oracle 提交流程

```go
type OracleSubmission struct {
    MarketID       string
    OutcomeResult  Outcome      // YES / NO / VOID
    SubmittedBy    UserID
    SubmittedAt    time.Time
    EvidenceURLs   []string     // 證據連結
    EvidenceHash   string       // 證據檔案 SHA256
    Notes          string
    Signature      string       // Operator 數位簽章
}
```

實作要求：
- 兩位 Oracle Operator **獨立**提交，結果一致才推進到 PENDING
- 結果不一致則自動進入 DISPUTED
- 提交 UI 強制螢幕截圖、貼證據連結，輔助 Operator 不漏項

### 4.3 Oracle 證據保存

證據檔案上傳至 R2 Object Lock 桶（WORM），路徑：

```
oracle-evidence/{market_id}/{submission_id}/
  ├── screenshot_001.png
  ├── source_url.txt
  ├── manifest.json   ← 含所有檔案 hash
  └── signature.bin
```

保留期：**5 年**。

## 5. 申訴窗口（Dispute Window）

### 5.1 公告格式

PENDING 進入時對所有相關玩家（持倉者）推播：

```
✉ Email + 站內訊息 + WS 推播
標題: 市場「{market.title}」初步判定結果
內容:
  初判結果: YES（YES 持有者中獎）
  判定依據: {oracle 提供的證據連結}
  申訴期限: 2026-05-06 14:30:00 UTC（24 小時內）
  申訴方式: [按鈕] 提出申訴
```

### 5.2 申訴規則

- 任何持有 ≥ 1 股相關 outcome 的玩家可申訴
- 每位玩家**每市場限申訴一次**
- 申訴需提供書面理由 + 反對證據（必填）
- 累計申訴比例 ≥ 5% 持倉者 OR 申訴金額 ≥ 平台 Top-5% 之一 → 自動進入 DISPUTED
- 累計申訴 < 5% 但有任一申訴含**新證據** → Compliance Officer 人工 review

### 5.3 申訴介面 API

```
POST /api/v1/markets/{market_id}/disputes
Body:
{
  "reason": "...",
  "evidence_urls": ["...", "..."],
  "claimed_outcome": "NO"
}

Response 201:
{
  "dispute_id": "disp_...",
  "status": "PENDING_REVIEW",
  "deadline": "..."
}
```

申訴期間市場狀態維持 PENDING，**不執行帳本結算**。

## 6. 爭議裁決（DISPUTED）

### 6.1 合議制

3 名 Dispute Adjudicator 獨立投票：

| 投票數 | 結論 |
|--------|------|
| 3:0 | 直接通過 |
| 2:1 | 通過 + 紀錄反對意見 |
| 1:1:1（三派分立）| 升級至 Compliance Officer 拍板 |

### 6.2 可能裁決結果

| 結果 | 後續動作 |
|------|---------|
| `UPHOLD_INITIAL` | 維持初判，進入 SETTLING |
| `OVERTURN` | 改判另一方獲勝，進入 SETTLING |
| `VOID_MARKET` | 市場無效，進入 CANCELLED，所有持倉原價退回 |
| `EXTEND_DISPUTE` | 延長 review 至最多 7 天，期間市場凍結 |

### 6.3 紀錄要求

每筆裁決寫入 `dispute_resolutions` 表：

```sql
CREATE TABLE dispute_resolutions (
    id              BIGSERIAL PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL,
    decision        VARCHAR(32) NOT NULL,
    voted_by        JSONB NOT NULL,         -- [{user_id, vote, reason, ts}]
    voted_at        TIMESTAMPTZ NOT NULL,
    final_signoff_by BIGINT,                -- Compliance Officer
    final_signoff_at TIMESTAMPTZ,
    audit_hash      CHAR(64) NOT NULL       -- 不可竄改 hash
);
```

## 7. 結算執行（SETTLING）

### 7.1 結算批次

當市場進入 SETTLING：

1. **凍結**所有相關持倉（不可賣出、轉移）
2. 啟動 Settlement Worker
3. 依持倉表逐筆計算結算金額：
   ```
   YES 中獎: payout = qty_yes * 1.00 點
   NO 中獎: payout = qty_no  * 1.00 點
   平局/取消: payout = average_buy_price * qty
   ```
4. 寫入帳本 `settle` transaction（見 SPEC-002 §4.4.4）
5. 發送站內訊息與推播通知
6. 市場狀態 → SETTLED

### 7.2 結算手續費

- 預設不收結算費（手續費已在交易階段收取）
- 例外：平台公告之大型賽事可加收 0.5%「結算手續費」（必須事前公告）

### 7.3 結算原子性

整個結算 transaction 必須在**單一資料庫 transaction** 內完成。若中途失敗：

- 全部 rollback
- 觸發 P0 告警
- 改為人工 batch（一次處理一筆持倉）並由 Settlement Reviewer 簽核

### 7.4 大型市場的批次處理

當持倉數 > 10,000 時：

- 拆分為多個 batch（每批 1000 筆）
- 每批一個 transaction，互相獨立
- 用 saga pattern 確保任何 batch 失敗時可逐批 retry
- 每批之間進行 Σ(payout) 與預期值對帳

## 8. 取消（CANCELLED）

### 8.1 觸發條件

- Oracle 判定 `VOID`（事件未發生 / 結果無法判定）
- Dispute 裁決為 `VOID_MARKET`
- 平台主動取消（極端情境，需 Compliance Officer 簽核）

### 8.2 取消結算邏輯

> **核心原則**：取消後，每位玩家**取回其持倉的加權平均建倉成本**。

```python
for user, position in market.positions:
    avg_cost = position.total_cost / position.qty
    refund = avg_cost * position.qty
    credit_to_wallet(user, refund)
```

### 8.3 手續費處理

> 取消市場不退手續費（已成交的交易雙方 + 平台拿到 fee 屬於既定事實）

例外：若是「市場創建錯誤 / 系統故障」導致取消，平台**自願**退手續費（從 Revenue.TradingFee 反沖至玩家）。

## 9. 特殊情境處理

### 9.1 結算前撤回所有未成交訂單

market.ClosesAt 到達時：
- 所有 ACTIVE / PARTIAL 訂單**全部 CANCEL**
- 鎖倉 reverse 回 wallet（依 SPEC-002 idempotent 處理）
- 玩家收到「市場關閉、未成交訂單退款」通知

### 9.2 結算延期

若 Oracle 在 `market.SettlesAt + 6h` 仍未提交結果：

- 市場狀態維持 CLOSED
- 觸發 P1 告警
- 自動延長 24 小時，最多延長 3 次
- 仍未獲結果 → 自動 CANCELLED

### 9.3 部分結果 / 平局

某些市場可能設計為 `Outcome ∈ {YES, NO, TIE}`（如：足球比賽）。

> **首發版不支援三元市場**，僅二元 YES/NO。若必須處理「平局」事件：

- 市場創建時禁止此類含平局可能的事件，**改設為兩個獨立二元市場**
  - 「主隊勝？」YES/NO
  - 「客隊勝？」YES/NO
  - 平局 = 兩個市場都 NO
- 若已創建錯誤事件 → 人工進入 VOID_MARKET 處理

### 9.4 玩家在結算前自我排除

若玩家在結算發生前永久封號（self-exclusion）：

- 持倉**仍正常結算**（不能因封號而沒收玩家應得收益）
- 結算後點數計入該玩家的「凍結錢包」
- 玩家若日後解除排除（最早 90 天後）可恢復點數

## 10. 結算事件流（API / WS 推播）

### 10.1 玩家可訂閱

```
ws://api.../ws/markets/{market_id}/status

← state_change: { "status": "PENDING", "result": "YES", "evidence": [...], "dispute_until": "..." }
← state_change: { "status": "SETTLING" }
← state_change: { "status": "SETTLED", "your_payout": 6500 }
```

### 10.2 公開結算列表

```
GET /api/v1/settlements?since=2026-05-01
→ 列出指定日期後的所有結算市場、結果、總成交量
（無玩家身份 PII，純公開資料）
```

## 11. 反作弊與洗錢監測整合

### 11.1 結算大戶監控

任一玩家在單一市場結算 payout > USD 50,000 等值：

- 自動觸發 EDD（Enhanced Due Diligence）
- 點數凍結至 KYC 強化驗證通過
- 大筆中獎客戶之歷史交易模式人工檢視（防共謀）

### 11.2 異常結算偵測

- 同一 IP / 裝置指紋 / 銀行帳戶之多帳號於同一市場一致下注 → 標記
- 結算後玩家立即試圖儲值或創建多筆小額交易 → 標記
- 標記之玩家 SAR（Suspicious Activity Report）自動產生

## 12. 性能與可靠性

| 指標 | 目標 |
|------|------|
| Oracle 提交 → PENDING 狀態變更延遲 | < 5 秒 |
| 申訴 API 回應 | P99 < 200ms |
| 結算批次（< 10k 持倉）完成時間 | < 60 秒 |
| 結算批次（10k–100k 持倉）完成時間 | < 10 分鐘 |
| 結算 transaction 失敗自動 retry | 最多 3 次，間隔 30s |
| 申訴窗口公告送達率 | 24h 內 > 99.9% |

## 13. 安全要求

- Oracle / Adjudicator 帳號**強制 hardware MFA**（YubiKey 或 TOTP）
- 結算 API 寫操作要求 step-up auth（每 30 分鐘重新 MFA）
- 結算操作 IP whitelist（內部 VPN 或固定辦公室 IP）
- 操作日誌 append-only + Real-time forwarding to SIEM

## 14. 驗收條件

- [ ] 完整狀態機 transition 表（13 種狀態變更）皆有自動測試
- [ ] 模擬 100k 持倉結算可在 10 分鐘內完成
- [ ] Oracle 雙簽不一致觸發 DISPUTED 流程驗證
- [ ] 申訴窗口公告 24h 內送達率 > 99.9%
- [ ] DR 演練：Settlement Worker crash 後自動 resume，無重複結算
- [ ] Multi-sig 簽核 UI 與後端強制 2-of-N 完全執行

## 15. Open Questions

1. 是否提供 API 給第三方 Oracle（Chainlink / UMA Optimistic Oracle）？目前為純人工，但成熟後可考慮自動化。
2. 申訴窗口固定 24 小時，還是依市場類型可變？短期市場（< 1 小時）是否縮短至 1 小時？
3. 「平局退款」的計算基準是否使用 VWAP（成交量加權平均價）而非每位玩家自身建倉均價？
4. 結算手續費是否預設為 0，或保留 0.5% 上限？
5. 大額中獎玩家的 EDD 強制門檻如何訂定？依執照地法規。
