# SPEC-008 — 災備與高可用（Disaster Recovery & HA）

> 優先級：**P1**
> 版本：v0.2.0 · 最後更新：2026-05-07
> 依賴：SPEC-002, SPEC-003, SPEC-007
> 被依賴：SPEC-013

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
| 客戶 KYC 資料 | **0**（R2 Object Lock 雙寫 EU + Singapore）| **0**（R2 全球複製，呼應 SPEC-001 §4.1）| < 1 小時 | P1 |
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
            │                                                      │
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
- 快取資料丟失可接受（重建自 PG）；Lock / Idempotency key 不可丟，存於 PG

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

### 7.1 Web 客戶端

- WebSocket 斷線自動重連（exponential backoff）
- API 請求自動 retry（GET 冪等；POST 帶 idempotency-key）
- 客戶端緩存最近狀態，重連後 reconcile

### 7.2 行動端（如有 LIFF）

- 與 Web 同樣自動重連
- 離線狀態 UI 提示
- 訂單操作必透過 server，不在 client 擱置

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

需 **2 of 3** 高權限角色（CTO / Compliance / Senior Engineer）共同解除。

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
5. 監管要求最低備份保留期是多久？（依執照地）
