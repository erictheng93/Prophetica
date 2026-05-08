# SPEC-008 — 災備與高可用（Disaster Recovery & HA）

> 優先級：**P1**
> 版本：v0.2.1 · 最後更新：2026-05-07
> 依賴：SPEC-002, SPEC-003, SPEC-007, SPEC-010
> 被依賴：SPEC-013

> **v0.2.1 變更（F-1 ~ F-5 縫補）**：(1) **F-1**：§9.3 Kill switch 解除明示 cross-ref 至 SPEC-010 §9.3 **MS-9**（multi-sig 中央表為單一真相來源），admin UI 不得自刻一套流程；(2) **F-2**：§7.1 / §7.2 客戶端容錯由「描述性敘述」升級為**可執行技術契約**（exponential backoff 上下限 + jitter、sequence-number reconcile、Idempotency-Key 對齊 SPEC-002 §6.1、retry budget 呼應 SPEC-007 §6.4.4 反 pattern）；(3) **F-3**：§13 OQ #5 移除（備份保留期改由本檔 §6.1 cold archive 策略與業主 / 運營要求覆蓋）；(4) **F-4**：§5.2 補 inline 註明「Lock / Idempotency 落 PG，**不**存 Redis」呼應 SPEC-002 §6.1；(5) **F-5**：§3.1 補 Cloudflare Edge layer 的 fallback semantics 待 PoC-B 完成後回填。
>
> **v0.2.0 變更（T-2 RPO 拆兩層）**：§2 RTO/RPO 表將 RPO 拆為「Region-local RPO（Tokyo 同 region synchronous standby + WAL fdatasync + Kafka acks=all）」+「Cross-region RPO ≤ 5 秒（async streaming replication 至 Singapore DR）」雙欄，消除原 v0.1「RPO=0」與 §3 跨區 async replication 之物理矛盾；新增 §2.1 Region-local RPO=0 三重保證實作要求 + §2.2 Cross-region RPO ≤ 5 秒 SLA 邊界與玩家通知策略。

---

## 1. Objective

確保任一單點故障（機器、機房、區域、第三方供應商）不會造成資料遺失或長時間服務中斷，並建立可被監管稽核的災備演練紀錄。

## 2. RTO / RPO 目標

> **RPO 分兩層**（T-2，2026-05-07 修正）：原本「帳本 RPO = 0」與 §3.1 / §5.1 之 async streaming replication（跨區延遲 ~70-100ms）物理矛盾；本表將 RPO 拆為「**Region-local RPO**」與「**Cross-region RPO**」兩欄。Tokyo Primary 之同 region synchronous standby 提供 RPO=0，跨區 Singapore DR 為 async replication 提供 RPO ≤ 5 秒。Failover 至 DR 區接受最多 5 秒資料丟失之 SLA。

| 服務 | Region-local RPO（Tokyo 同 region）| Cross-region RPO（Tokyo → Singapore DR）| RTO（恢復時間）| 優先級 |
|------|---------------------------------|-------------------------------------|---------------|--------|
| 帳本（Ledger）| **0**（synchronous standby + WAL fdatasync + Kafka acks=all）| **≤ 5 秒**（async streaming replication）| < 5 分鐘 | P0 |
| 撮合引擎 | < 1 秒（per-market WAL + Kafka acks=all）| ≤ 5 秒（Kafka MirrorMaker 2 lag）| < 30 秒 / 市場 | P0 |
| 玩家錢包 | **0**（同 ledger）| **≤ 5 秒**（同 ledger）| < 5 分鐘 | P0 |
| 結算服務 | < 1 分鐘（saga checkpoints）| ≤ 5 分鐘 | < 30 分鐘 | P1 |
| 客戶 KYC 資料 | **0**（R2 Object Lock 雙寫 EU + Singapore）| **0**（R2 全球複製）| < 1 小時 | P1 |
| 撮合事件流（WAL）| **0**（fdatasync 每 100 events 或 50ms）| ≤ 5 秒（Kafka MirrorMaker）| < 5 分鐘 | P0 |
| WebSocket 推播 | < 5 秒（in-memory subscriber state）| < 30 秒（reconnect + gap detect）| < 5 分鐘 | P1 |
| 行銷頁面 | < 1 小時 | < 1 小時 | < 2 小時 | P3 |
| 報表 / 分析 | < 24 小時 | < 24 小時 | < 24 小時 | P3 |

### 2.1 Region-local RPO=0 實作要求（撮合 + ledger + wallet）

「Region-local RPO=0」之達成需要**雙重保證**，缺一不可：

1. **PostgreSQL synchronous standby（同 region）**：Tokyo Primary 至少配置一個 same-region synchronous standby（`synchronous_commit = on` + `synchronous_standby_names = 'standby1'`），commit 必須等該 standby 確認 WAL 接收才返回。失去 standby 時系統進入 fail-closed（Primary 拒寫）而非 fail-open。
2. **撮合引擎 WAL fdatasync**：per-market WAL 每 100 events 或 50ms（取先到者）`fdatasync()`，呼應 SPEC-003 §8.2。
3. **Kafka acks=all + replication.factor=3**：撮合事件 publish 至 Kafka 採同步多副本確認，呼應 SPEC-003 §8.1。

任一機制失效 → P0 告警 + Primary 進入 read-only mode 等待修復；不允許「降級為 RPO ≤ 5 秒同 region」之操作。

### 2.2 Cross-region RPO ≤ 5 秒之 SLA 邊界

- DR failover 至 Singapore 時，**接受**最近 ≤ 5 秒之撮合事件 / ledger entries / wallet 異動丟失
- 對玩家通知策略：failover 後若偵測到丟失，自動寄出「事故說明 + 受影響交易明細」郵件，並由客服 follow up 個案處理
- 此 5 秒 RPO 為**業務 SLA 上限**；技術 SLO（async replication lag）平時應 < 1 秒，> 3 秒觸發 P1 告警，> 5 秒持續觸發 P0 + DR 區介入

## 3. 多區架構

### 3.1 區域佈局

```
            ┌──────────────── Cloudflare Anycast ────────────────┐
            │           （含 Edge runtime PoC 之靜態 / SSR 層）        │
            │           （PoC-B 結果待 SPEC-014 §3.2.1 回填）          │
            ▼                                                      ▼
  ┌──────────────────┐                              ┌──────────────────┐
  │ Primary Region   │  ←── Streaming Replication →│ DR Region        │
  │ Linode Tokyo     │                              │ AWS Singapore    │
  │ • API Gateway    │                              │ • API Gateway    │
  │ • Match Engine   │                              │ • Match Engine   │
  │ • Ledger Service │                              │ • Ledger Service │
  │ • PostgreSQL P   │                              │ • PostgreSQL R   │
  │ • Redis Cluster  │                              │ • Redis Cluster  │
  │ • Kafka Cluster  │                              │ • Kafka Cluster  │
  └──────────────────┘                              └──────────────────┘
```

> **F-5 待補**：Cloudflare Edge runtime（SPEC-014 §3.2.1 PoC-B）通過後，需回填以下 fallback semantics：(a) 行銷頁面 / 公開市場列表之 Edge cache → Origin 退路；(b) Edge KV 與 Origin DB 之 staleness window；(c) Edge runtime 服務不可用時是否降級至直連 Origin。低優先；可隨 PoC-B 實機驗證結果一同 commit。

### 3.2 Active-Active vs Active-Passive

決策：**Active-Passive**（DR 為熱備，不接生產流量）

理由：
- 撮合引擎為單寫者，雙寫會破壞順序保證
- 帳本必須單一真相來源
- 跨區一致性成本高於切換成本

DR 區仍持續：
- PostgreSQL streaming replication（< 1 秒延遲）
- Kafka MirrorMaker 2 同步
- 應用層 standby 不啟動 ingress

### 3.3 Failover 演練

每季一次計畫性切換，演練：

1. **計畫性切換**：通告維護窗、切流量、切 DB primary、驗證
2. **災難演練**：模擬 Tokyo 區域不可用，自動 failover、驗證 RTO

## 4. 撮合引擎災備

### 4.1 寫入路徑（持久性層級）

```
Order → Engine in-memory book
           │
           ├─→ WAL (local disk, fdatasync)         ← 主持久性 (P0)
           ├─→ Kafka (acks=all, replication=3)     ← 跨區持久性 (P0)
           ├─→ Snapshot every 60s (S3/R2)          ← 快速恢復用 (P1)
           └─→ Trade events to consumers           ← 業務下游 (P0)
```

### 4.2 故障情境與恢復

| 情境 | 恢復步驟 | RTO |
|------|---------|-----|
| Engine pod crash | K8s 重啟 → 從 snapshot + WAL 恢復 → ingress 開放 | < 30 秒 |
| 整機停電 | hostNetwork pod 在另一節點重啟 → 從 Kafka replay | < 60 秒 |
| 整機房故障 | DR 區的 standby engine 啟動 → 從 Kafka MirrorMaker replay | < 5 分鐘 |
| WAL 磁碟故障 | engine 拒收新 order → 從 Kafka 重建 in-memory state | < 10 分鐘 |
| Kafka 集群整體故障 | engine 進入 read-only mode → 等待 Kafka 恢復 | depends |

### 4.3 為何 WAL + Kafka 雙寫？

- WAL 提供本地零延遲持久性（不依賴網路）
- Kafka 提供跨機器、跨區持久性
- 任一單點故障不會丟資料

## 5. 資料庫災備

### 5.1 PostgreSQL 配置

```
Primary (Tokyo) ──streaming replication──→ Standby (Tokyo Replica)
       │                                          │
       │                                          └→ 讀取流量分散
       │
       └─physical streaming replication──→ DR Standby (Singapore)
```

- WAL 連續歸檔至 R2（保留 30 天）
- 快照（pg_basebackup）每日，保留 14 天
- PITR（Point-in-Time Recovery）能力：可回到任一時間點

### 5.2 Redis 配置

```
Tokyo Redis Cluster (3 master, 3 replica) ←─ async replication ─→ Singapore Redis (passive)
```

- AOF + RDB 雙重持久化
- AOF fsync=everysec（性能與持久性平衡）
- 快取資料丟失可接受（重建自 PG）
- **Lock / Idempotency key 不存 Redis**：呼應 SPEC-002 §6.1，所有 `Idempotency-Key` 與分散式 lock 落 PostgreSQL `idempotency_records` 表 / `pg_advisory_lock`；Redis **僅**作為熱資料 cache（市場列表 / WS subscriber state / leaderboard）。理由：Redis AOF fsync=everysec 在 worst case 可丟失最近 1 秒寫入，與 SPEC-002 §6 帳本入口的「精確一次」契約不相容

### 5.3 對帳

DR 區每日對帳：

- DR PG 與 Primary PG 的 entries 數一致
- DR Kafka offsets 進度差異 < 1 秒
- 不一致 → P0 告警

## 6. 備份策略

### 6.1 三層備份

| 層級 | 物件 | 頻率 | 保留 | 儲存位置 |
|------|------|------|------|---------|
| Hot Standby | DB / Redis / Kafka | 即時 | 持續 | DR 區 |
| Daily Snapshot | DB pg_basebackup | 每日 | 30 天 | R2 + S3 雙寫 |
| Cold Archive | Critical 帳本快照 | 每月 | 7 年 | R2 Object Lock + Glacier |

### 6.2 備份驗證

- 每週一次自動 restore 驗證（取最新備份至沙盒環境，跑 schema check）
- 每月一次完整 restore 演練
- 不通過驗證的備份標記為 INVALID，告警

## 7. 客戶端容錯

> **v0.2.1 升級**：本節由 v0.2.0 之描述性敘述（exponential backoff / reconcile / idempotency-key 三個名詞）升級為**可執行技術契約**。前端實作（apps/web、apps/liff）必須照本節 §7.1 / §7.2 / §7.3 落地，禁止三人三套；契約變更須 PR 修改本節 + 對應前端 lib 同步更新。

### 7.1 Web 客戶端 — WebSocket 重連契約

| 項目 | 契約 | 理由 |
|------|------|------|
| 演算法 | Exponential backoff with full jitter | 避免 thundering herd（後端故障恢復時） |
| 初始延遲 | **500 ms** | 短於人眼可感知斷線（< 1s）|
| 倍率 | **2.0** | 標準 binary backoff |
| Jitter | **±20%（uniform random）** | 分散重連 spike |
| Cap（單次最大延遲）| **30 s** | 對齊 §8 第三方 DR 切換 TTL；超過此值已脫離「容錯」進入「降級」 |
| Max attempts | **30 次** | 約 ~10 分鐘總預算（含 jitter）；超過後進入 §7.3 fail-soft 模式 |
| Heartbeat | client → server `ping` 每 **15 s**；server 30 s 無 pong → 主動關閉 + 觸發重連 | 偵測「TCP 半開連線」（NAT timeout / network partition）|

實作參考：`apps/web/lib/ws-client.ts`（v0.2.1 隨 SPEC patch 落地）。

### 7.2 Web 客戶端 — API retry 與 Idempotency-Key 契約

| 方法 | retry 行為 | Idempotency-Key 要求 |
|------|-----------|---------------------|
| `GET` | 自動 retry up to **3 次**（連線級錯誤 / 5xx）；exponential 200ms / 400ms / 800ms | 不需要（讀操作天然冪等）|
| `POST` / `PUT` / `DELETE`（寫帳本）| **必須**帶 `Idempotency-Key` header；retry 行為由 SPEC-002 §6.1 backend `idempotency_records` 處理；client 端 retry up to **3 次**，**重用同一 key** | **必須**：UUID v7（client-generated，128 bit）|
| `POST` 非帳本（如 health probe）| 不 retry | 不需要 |

Idempotency-Key 格式：

- **算法**：UUID v7（time-ordered，48 bit unix ms timestamp + 74 bit randomness）
- **格式**：標準 RFC 4122 字串（36 字元含 hyphen）
- **生命週期**：每個玩家行為（單次下單、單次儲值、單次申訴）生成 1 個 key，retry 全程重用
- **觀測性**：寫入 OTel span attribute `idempotency.key`，呼應 SPEC-007 §6.4 latency budget 追蹤

### 7.3 Web 客戶端 — Reconcile 與 fail-soft

**Reconcile 機制**（重連成功後）：

1. WS 訊息 payload 必須帶 monotonic `seq` field（per-subscription 連續整數）
2. Client 偵測 `seq` gap → 主動發 `GET /api/markets/{id}/snapshot?from_seq=N` 補洞
3. Backend 應在 60 秒內保留 per-subscription event tail（用於 reconcile）
4. Reconcile 失敗（gap 過大 / tail 過期）→ 客戶端強制重新訂閱

**Fail-soft 模式**（max attempts 用盡後）：

- 顯示「**連線已中斷**」橫幅（呼應 §9.2 HALTED 狀態 UI 同一 component；SPEC-008 v0.2.1 收斂為單一 `<ConnectionStatusBanner>`）
- 自動降級為 polling：`GET /api/markets/{id}/book` 每 5 秒
- **禁止**繼續送下單請求（client 主動 disable 下單按鈕，避免 backend 故障期間累積大量待 retry POST）
- 提供「**手動重試連線**」按鈕讓玩家主動觸發新一輪 reconnect

**Retry budget 反 pattern 警示**（呼應 SPEC-007 §6.4.4）：

- ❌ 禁止：「永久重試」「無上限 retry」「retry 失敗時把延遲歸零重新算」
- ❌ 禁止：retry storm — 同一玩家同一秒內超過 3 個 retry POST（client 端 token bucket 限流）
- ❌ 禁止：對 4xx 錯誤 retry（這是 client bug，不是網路問題）

### 7.4 行動端（LIFF）

- WebSocket 重連契約**完全繼承** §7.1
- Idempotency-Key 契約**完全繼承** §7.2（LIFF SDK 由 `apps/liff/lib/` 共用 `apps/web/lib/idempotency.ts`）
- LINE app 切到背景 → WS 由 OS 自動斷開；返回前景時觸發重連，**不**累計成 max attempts
- 離線狀態 UI 提示：用 `navigator.onLine` 偵測；offline 時直接進入 §7.3 fail-soft，不消耗 retry budget
- **訂單操作必透過 server，不在 client 擱置**

## 8. 第三方依賴災備

| 依賴 | 替代方案 | 切換策略 |
|------|---------|---------|
| Cloudflare | AWS CloudFront | DNS 切換，TTL 60s |
| Linode | AWS / GCP Tokyo | 已部署 Singapore，可立即接管 |
| 支付 Gateway（綠界）| 藍新 / Stripe | 玩家可選通路；自動降級至備援 |
| KYC SaaS（SumSub）| Onfido | 同時整合 2 家，主備切換 |
| Email（SendGrid）| AWS SES | 配置 fallback |
| SMS（Twilio）| 任一備援 | 同上 |

## 9. 緊急停盤（Kill Switch）

### 9.1 觸發條件

- 帳本失衡偵測
- 大量異常下注（疑似系統 bug 或攻擊）
- 監管緊急要求
- 平台主動公告維護

### 9.2 停盤動作

`POST /api/internal/admin/halt` → 設定 feature flag `engine.halted=true`

- 撮合引擎拒收新 order，但繼續 broadcast 既有狀態
- WS 推播狀態為 HALTED
- 結算暫停
- 通知所有玩家 + 公告原因
- 預期恢復時間

### 9.3 解除

**呼應 SPEC-010 §9.3 MS-9**（multi-sig 治理中央表）：Kill switch 解除需 **2-of-3** CTO / Compliance / Senior Engineer 共同簽核。

> **重要 governance 約束**（v0.2.1 補）：admin / Compliance UI **不得**為 MS-9 自刻一套簽核流程；必須使用 SPEC-010 §9.3.3 規範之**單一** multi-sig component（schema 由 §9.3.1 規則總表驅動）。簽核者強制 hardware MFA（YubiKey / Passkey），對應 step-up MFA 過去 30 分鐘內仍有效。任何對 MS-9 簽核要求之變更（如改成 3-of-3、加入 RG Officer），須先 PR 修 SPEC-010 §9.3.1，本節僅為呼應引用。

解除動作必須寫入 `audit_logs`（呼應 SPEC-012 §5.3），`metadata.ms_id = "MS-9"`、`action = "kill_switch_release"`、`subject_id` 為觸發停盤之事故 ID。

## 10. 演練計畫（Drill Schedule）

| 演練類型 | 頻率 | 內容 |
|---------|------|------|
| Engine pod kill | 每週 | K8s 隨機 kill 一個 engine pod |
| DB primary failover | 每月 | 切換 primary，驗證 < 30 秒 RTO |
| 區域切換 | 每季 | 演練 Tokyo → Singapore |
| 完整 DR 演練 | 每年 | 模擬 Tokyo 全毀，從 zero 重建 |
| Tabletop 演練 | 每月 | 工程 + 法務 + 客服紙上推演 |

## 11. 演練紀錄要求

每次演練須產出：

- 演練計畫文件（事前）
- 實際執行紀錄（含時間軸、命中 / 偏離計畫）
- 改進建議（事後 24h 內）
- 監管可查閱（必要時）

## 12. 驗收條件

- [ ] 雙區部署完成，DR 區可獨立運行（無 Primary 仍可服務）
- [ ] PostgreSQL streaming replication 延遲 P99 < 1 秒
- [ ] Engine 從 snapshot + WAL 恢復演練通過 RTO
- [ ] 跨區 failover 演練通過 RTO
- [ ] Kill switch 完整測試（觸發、停盤行為、解除）
- [ ] 每週備份驗證腳本自動運行 + 失敗告警

## 13. Open Questions

1. DR 區是否使用 AWS（與 Linode 異質供應商）還是 Linode Singapore？同質風險 vs 維運複雜度
2. PostgreSQL 物理複製還是邏輯複製？跨雲建議邏輯複製
3. Kafka MirrorMaker 2 vs Confluent Replicator？
4. 緊急停盤是否同時影響 KYC / 客服流程？預設不影響，但需確認

> **OQ #5 已 Resolved（v0.2.1，2026-05-07）**：「最低備份保留期」由本檔 §6.1 cold archive 策略與 SPEC-000 §5 Q-2 業主 / 運營最終意見覆蓋；現有設計 §6.1 cold archive 7 年留有上修空間。本檔不再追蹤此議題。
