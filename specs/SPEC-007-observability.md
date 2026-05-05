# SPEC-007 — 可觀測性與監控（Observability & Monitoring）

> 優先級：**P1**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-003, SPEC-014
> 被依賴：SPEC-008, SPEC-013

---

## 1. Objective

建立金融級可觀測性系統，使工程團隊能在 5 分鐘內定位任何故障的根因，並提前 30 分鐘預警容量瓶頸。要求三柱齊全：**Metrics**（指標）、**Logging**（日誌）、**Tracing**（分散式追蹤），並建立明確的 SLO 與告警 Runbook。

## 2. 三柱架構

```
[ Application ]
    │
    ├─→ Metrics  → Prometheus (metrics scrape)        → Grafana
    │                  ↓
    │              Long-term: Mimir / VictoriaMetrics
    │
    ├─→ Logs     → Promtail / Vector → Loki           → Grafana
    │
    └─→ Traces   → OpenTelemetry SDK → OTel Collector → Tempo / Jaeger → Grafana

[ Alerts ]    ← Prometheus AlertManager → Slack / PagerDuty / Telegram
```

> 部署模型：**Grafana Cloud**（託管）vs **自建 LGTM stack**。MVP 用 Grafana Cloud，年營收破門檻後遷至自建。

## 3. Metrics

### 3.1 標準命名

```
proph_<domain>_<subject>_<unit>
```

範例：

```
proph_engine_orders_received_total              counter, labels: market_id, type
proph_engine_match_latency_seconds              histogram, labels: market_id
proph_engine_orderbook_depth                    gauge,  labels: market_id, side
proph_ledger_txn_committed_total                counter, labels: txn_type
proph_ledger_txn_failed_total                   counter, labels: txn_type, reason
proph_payment_gateway_request_seconds           histogram, labels: provider, status
proph_kyc_review_outcome_total                  counter, labels: provider, outcome
proph_ws_connected_clients                      gauge,  labels: shard
proph_db_pool_in_use                            gauge,  labels: db
proph_redis_op_latency_seconds                  histogram, labels: op
proph_settlement_jobs_running                   gauge
proph_settlement_payout_pts_total               counter, labels: market_id
proph_rg_score_distribution                     histogram
proph_aml_sar_created_total                     counter
proph_kyc_provider_webhook_received_total       counter, labels: provider, type
```

### 3.2 直方圖 bucket 規劃

撮合延遲（核心指標）：

```
[0.0001, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.025, 0.05, 0.1, 0.5, 1.0, 5.0]
```

API 延遲：

```
[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
```

### 3.3 業務指標

每個業務指標都應有 metric：

| 指標 | 用途 |
|------|------|
| `proph_dau_total` | 日活 |
| `proph_revenue_pts_total` | 平台收入點數 |
| `proph_market_open_count` | 開放市場數 |
| `proph_user_signup_total` | 註冊數 |
| `proph_user_kyc_passed_total` | KYC 通過數 |
| `proph_player_balance_pts_sum` | 玩家持有點數總和（負債）|
| `proph_player_locked_pts_sum` | 玩家鎖倉點數總和 |
| `proph_self_exclusion_active_total` | 自我排除人數 |

## 4. Logging

### 4.1 結構化 JSON

所有服務輸出 structured JSON：

```json
{
  "timestamp": "2026-05-05T12:34:56.789Z",
  "level": "info",
  "service": "matching-engine",
  "version": "1.2.3",
  "trace_id": "abc...",
  "span_id": "def...",
  "user_id": 42,           // 視 context 是否包含
  "market_id": "mkt_...",
  "event": "order.matched",
  "msg": "Order matched 50 of 100 qty",
  "duration_ms": 1.2,
  "...domain fields..."
}
```

### 4.2 日誌等級規範

| 等級 | 用途 | 範例 |
|------|------|------|
| `debug` | 開發排查，prod 預設不開 | 進入函式 |
| `info` | 業務流關鍵事件 | 訂單接收、撮合 |
| `warn` | 可容忍的異常 | retry 成功、降級 |
| `error` | 需要 oncall 注意 | 5xx、外部 API 失敗 |
| `critical` | 立即介入 | 帳本失衡、結算失敗 |

### 4.3 PII 處理

- **絕不**將 PII（姓名、證件號、卡號、手機）寫入日誌
- user_id 為唯一識別子（內部數字，不洩漏）
- 客服查詢 PII 走獨立稽核 API，存取留痕

### 4.4 採樣與保留

| 等級 | 採樣 | 保留期 |
|------|------|--------|
| debug | 1% | 1 天 |
| info | 100% | 30 天 |
| warn / error | 100% | 90 天 |
| critical | 100% | 1 年 + R2 歸檔 5 年 |

## 5. Tracing（分散式追蹤）

### 5.1 OpenTelemetry SDK

所有服務必須注入 OTel SDK：

- 入口（HTTP / gRPC / WS）自動產生 root span
- 跨服務調用透過 W3C `traceparent` header 傳遞
- DB 查詢自動產生 child span（含 SQL）
- 外部 API 自動 instrumented

### 5.2 採樣

- prod 預設 1% 採樣（避免儲存爆炸）
- error / critical span：100% 採樣
- 客服指定 user_id 查詢：100% 採樣 1 小時（debug 用）

### 5.3 必含 attributes

```
service.name           e.g. "matching-engine"
service.version
deployment.environment e.g. "prod" / "staging"
proph.market_id
proph.user_id
proph.order_id
proph.txn_id
```

## 6. SLO（Service Level Objectives）

### 6.1 SLO 定義表

| 服務 | SLI（量測） | SLO（目標）| 錯誤預算 / 月 |
|------|-----------|-----------|--------------|
| API Gateway | 5xx error rate | < 0.1% | 43 分鐘 |
| Matching Engine | match latency P99 | < 5ms | 7.2 hours of degraded |
| WebSocket | message delivery | > 99.9% | 43 分鐘 |
| Payment Gateway | webhook processing | < 30s P99 | 4 小時 |
| Settlement | batch completion | within 10min | 1 次 / 月 |
| KYC Webhook | processing | < 1min P99 | 4 小時 |

### 6.2 錯誤預算政策

- 每月初重置
- 超過 50% 預算：暫停非必要 release，優先穩定性
- 超過 100% 預算：強制凍結 release 至下月

### 6.3 SLO 看板

Grafana 統一 SLO Dashboard：

- 每個 SLO 即時錯誤預算進度條
- 30 / 90 / 365 天 burn rate
- 預算耗盡預測

## 7. 告警與 Runbook

### 7.1 告警分級

| 級別 | 通知管道 | 響應時間 |
|------|---------|---------|
| **P0** | PagerDuty + 電話 + Slack | < 5 分鐘 |
| **P1** | Slack #oncall + Email | < 30 分鐘 |
| **P2** | Slack #engineering | < 4 小時 |
| **P3** | Slack #monitoring（informational）| 工作日內 |

### 7.2 P0 告警清單（必須處理）

- 帳本失衡：`proph_ledger_txn_failed_total{reason="imbalance"} > 0`
- 撮合 P99 > 50ms 持續 1 分鐘
- API 5xx > 5% 持續 2 分鐘
- 支付 Gateway webhook 失敗 > 10 筆 / 分鐘
- 結算批次失敗
- WebSocket 連線數驟降 > 50%
- 託管銀行對帳差異 > USD 1,000
- 證件儲存桶寫入失敗
- KYC SaaS API 不可用 > 5 分鐘
- 制裁名單篩查 batch 失敗

### 7.3 Runbook 結構

每個告警必須有對應 Runbook，存於 `runbooks/` Repo：

```markdown
# Runbook: ledger-imbalance

## Alert
proph_ledger_txn_failed_total{reason="imbalance"} > 0

## Severity
P0

## Symptoms
某筆 ledger transaction 違反借貸平衡，被 DB trigger reject。

## Investigation
1. Grafana Loki 查 service=ledger level=critical txn_id={alert.txn_id}
2. PG 查 ledger_transactions WHERE id = {txn_id}
3. PG 查 ledger_entries WHERE txn_id = {txn_id}

## Resolution
1. 立即凍結出帳服務（feature flag: payment.disabled=true）
2. 計算差額來源
3. 由 Senior Engineer + Compliance 簽核 reversal txn

## Postmortem
24 小時內提交 RCA 文件至 docs/postmortems/
```

## 8. 健康檢查（Health Check）

### 8.1 三層 endpoint

```
GET /healthz/live      → 進程是否還活著（Liveness）
GET /healthz/ready     → 是否準備接流量（Readiness）
GET /healthz/startup   → 啟動是否完成（Startup）
```

### 8.2 readiness 判定

服務自身依賴的關鍵元件全綠才回 200：

- DB 連線可用
- Redis 連線可用
- Kafka producer 可發送
- 上游服務 ping 通

## 9. 容量規劃指標

每月分析：

| 指標 | 預警閾值 |
|------|---------|
| DB connection pool peak | > 70% |
| PostgreSQL disk | > 70% |
| Redis memory | > 60% |
| Kafka topic backlog | > 100k events |
| K8s node CPU avg | > 60% |
| Cloudflare egress | > 80% of plan |

## 10. 反作弊指標暴露

呼應 SPEC-010：

- `proph_security_self_trade_blocked_total`
- `proph_security_wash_trade_score_distribution`
- `proph_security_device_fingerprint_collision_total`
- `proph_security_login_anomaly_total`

## 11. 法遵指標暴露

呼應 SPEC-001 / SPEC-005 / SPEC-006：

- `proph_aml_sar_pending_total`
- `proph_kyc_tier_distribution`
- `proph_rg_high_risk_users_total`
- `proph_self_exclusion_active_total`

## 12. 驗收條件

- [ ] 所有服務注入 OTel SDK，trace 端到端可串接
- [ ] Prometheus 採集所有指標清單，無 missing
- [ ] Loki 接收所有結構化日誌，PII 採樣後無洩漏
- [ ] 主要 Dashboards 建立完成（API、Engine、Wallet、Settlement、Compliance）
- [ ] P0 告警全數綁定 PagerDuty 並通過 fire drill
- [ ] Runbook 100% P0 告警對應
- [ ] 模擬故障演練：每季至少一次

## 13. Open Questions

1. Grafana Cloud 還是自建？成本 vs 維運負擔
2. APM trace 採樣率：1% 還是 0.1%？
3. PII 在 trace 中如何 redact？OTel SDK processor？
4. 日誌中的 trace_id 是否寫入 PG（用於客服查詢）？
5. SLO 計算窗口：滾動 30 天還是日曆月？
