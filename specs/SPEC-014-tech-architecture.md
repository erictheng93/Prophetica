# SPEC-014 — 整體技術架構（Tech Architecture）

> 優先級：跨章節（被多份 SPEC 依賴）
> 版本：v0.1.2 · 最後更新：2026-05-05
>
> **v0.1.2 變更**：前端框架由 Nuxt 3 (Vue 3) 切換至 Next.js 15 (React 19)，元件庫採用 Cloudflare Kumo（`@cloudflare/kumo`，建立於 Base UI）。狀態管理 Pinia → Zustand；表單 VeeValidate → React Hook Form。理由詳見 `design/COMPONENTS.md`。

---

## 1. Objective

定版整個系統的服務拓撲、技術棧、Repo 結構、訊息流、跨服務協議。供所有 SPEC 引用。

## 2. 服務拓撲（High-level）

```
                                                ┌──────────────────┐
                                                │ Compliance UI    │
                                                │ Admin Console    │
                                                └────────┬─────────┘
                                                         │
                       ┌──────────── Cloudflare ────────┼──────────────────┐
                       │ (WAF, CDN, DNS, R2)            │                  │
                       │                                ▼                  │
   Player Browser ────►│ ── Next.js 15 SSR/RSC ──► API Gateway (Go/Echo) ─►│
                       │                                │                  │
                       │ ── WS connections ──────────► WS Gateway (Go) ◄───│
                       │                                                   │
                       └───────────────────────────────────────────────────┘
                                                         │
                ┌────────────────────────────────────────┼────────────────────────┐
                │                                        │                        │
                ▼                                        ▼                        ▼
   ┌───────────────────┐               ┌───────────────────┐         ┌──────────────────┐
   │ User Service      │               │ Wallet / Ledger   │         │ Market Service   │
   │ (auth, KYC,       │               │ (PostgreSQL       │         │ (CRUD, Oracle    │
   │  RG, profile)     │               │  double-entry)    │         │  submissions)    │
   └─────────┬─────────┘               └─────────┬─────────┘         └────────┬─────────┘
             │                                    │                            │
             ▼                                    ▼                            ▼
   ┌───────────────────┐               ┌───────────────────┐         ┌──────────────────┐
   │ KYC Webhook Recv  │               │ Settlement Worker │         │ Risk / Anti-Fraud│
   └─────────┬─────────┘               └─────────┬─────────┘         └────────┬─────────┘
             │                                    │                            │
             └─────────────────┬──────────────────┴────────────────────────────┘
                               │ events
                               ▼
                       ┌────────────────┐
                       │ Kafka Cluster  │ ← all event sourcing
                       └─┬────────┬─────┘
                         │        │
            ┌────────────┘        └─────────────┐
            │                                    │
            ▼                                    ▼
   ┌──────────────────┐                  ┌──────────────────┐
   │ Matching Engine  │                  │ Position Service │
   │ (Go single-      │                  │ (持倉管理)        │
   │  writer per mkt) │                  └──────────────────┘
   │                  │
   │ + WAL local disk │
   │ + Snapshot R2    │
   └──────────────────┘

   PostgreSQL (Tokyo Primary + Replica + Singapore DR)
   Redis Cluster (緩存、Rate limit、Session)
   ClickHouse / TimescaleDB (tick data, K 線, 分析)
   Vault (secrets)
   Prometheus + Loki + Tempo + Grafana (observability)
```

## 3. 技術棧定版

### 3.1 後端

| 元件 | 選擇 | 替代 | 理由 |
|------|------|------|------|
| Matching Engine | **Go 1.22+** | Rust / C++ | 開發速度 vs 性能平衡 |
| API Gateway | **Go + Echo** | Gin / Fiber | 標準 |
| Event bus | **Kafka (Confluent or self-hosted KRaft)** | NATS JetStream | 業界標準、生態強 |
| Primary DB | **PostgreSQL 16** | MySQL | ACID + 進階功能 |
| Cache | **Redis 7 Cluster** | KeyDB | 標準 |
| Time-series | **TimescaleDB** | ClickHouse | PG 整合好 |
| Search (用戶查詢市場) | **Meilisearch** | Elasticsearch | 輕量足夠 |
| Object Store | **Cloudflare R2** | AWS S3 | 成本 + 整合 Cloudflare |
| KYC SaaS | **SumSub** | Onfido | 性價比 |
| Secrets | **HashiCorp Vault** | AWS Secrets | 標準 |
| Email | **SendGrid** | AWS SES | 簡單 |
| SMS | **Twilio** | Vonage | 標準 |

### 3.2 前端

| 元件 | 選擇 | 替代 | 理由 |
|------|------|------|------|
| Web App | **Next.js 15 (React 19, App Router)** | Remix | Kumo 為 React-only 元件庫；RSC + SSR 對 SEO 與初載速度有利 |
| UI 元件庫 | **`@cloudflare/kumo`**（建立於 Base UI） | Radix UI / shadcn/ui | 業主指定；無障礙開箱即用，CSS variables 易主題化 |
| Icon | **`@phosphor-icons/react`** | lucide-react | Kumo peer 依賴 |
| 狀態管理 | **Zustand** | Redux Toolkit / Jotai | React 18+ 友善、bundle 小、與 RSC 共存無痛 |
| Data Fetching | **TanStack Query v5** | SWR | 撮合資料快取 + 樂觀更新 |
| Style | **TailwindCSS** | — | 與 Kumo CSS variables 並用 |
| WS Client | **native WebSocket** + custom reconnect | Phoenix Channels | 簡單可控 |
| Charts | **Lightweight Charts (TradingView)** | Recharts | 高性能訂單簿圖 |
| LIFF / LINE | **LIFF SDK**（需 React wrapper） | — | PRD 確認 |
| 表單 | **React Hook Form + Zod** | — | RHF 為 React 標準；Zod schema 與後端共用 |
| 行為追蹤 | **Mixpanel + 自建 tracking** | Amplitude | 預測市場專屬 |

### 3.3 基礎設施

| 元件 | 選擇 |
|------|------|
| 容器 | Docker + Distroless base |
| 編排 | Kubernetes (Linode LKE 主，AWS EKS DR) |
| GitOps | ArgoCD |
| Service Mesh | **不使用**（避免延遲）|
| IaC | Terraform |
| Container Registry | GitHub Container Registry |
| Monitor | Prometheus + Grafana Cloud |
| Trace | OpenTelemetry → Tempo |
| Log | Loki / Promtail |
| CI/CD | GitHub Actions |

## 4. 服務清單與職責

| Service | 語言 | 職責 | 部署規模 |
|---------|------|------|---------|
| `api-gateway` | Go | 邊緣 API、認證、路由 | 3+ replicas |
| `ws-gateway` | Go | WebSocket 連線、推播扇出 | 5+ replicas |
| `user-service` | Go | 註冊、登入、profile、TOTP | 3+ replicas |
| `kyc-service` | Go | KYC 流程編排、SaaS 整合、webhook | 2+ replicas |
| `wallet-ledger-service` | Go | 雙簿記帳本、idempotency | 3+ replicas + leader |
| `market-service` | Go | 市場 CRUD、Oracle、結算狀態 | 2+ replicas |
| `matching-engine` | Go | 撮合引擎（per-market goroutine）| DaemonSet, hostNetwork |
| `position-service` | Go | 持倉聚合 | 3+ replicas |
| `settlement-worker` | Go | 結算批次處理 | 1 active + standby |
| `risk-antifraud-service` | Go | 風控規則、行為分析 | 2+ replicas |
| `referral-service` | Go | 推薦關係、佣金計算 | 1+ replica |
| `notification-service` | Go | Email / SMS / Push / 站內訊息 | 2+ replicas |
| `web-app` | Next.js 15 (React 19) | 玩家前台 | Cloudflare Pages（`@cloudflare/next-on-pages`） |
| `admin-app` | Next.js 15 (React 19) | 內部後台 | 私網 + mTLS |

## 5. Repository 結構（Monorepo）

採用 **Turborepo + pnpm workspaces** + **Go workspace（go.work）**

> 此處為**規範來源（normative）**；SPEC-017 §2 為 bootstrap 後的實際 file tree，必須與本節一致。

```
prophetica/
├── apps/
│   ├── web/                    Next.js 15 玩家前台（React + Kumo）
│   ├── admin/                  內部後台
│   └── liff/                   LINE LIFF mini app
├── services/
│   ├── api-gateway/            Go
│   ├── ws-gateway/             Go
│   ├── user-service/           Go
│   ├── kyc-service/            Go
│   ├── wallet-ledger-service/  Go
│   ├── market-service/         Go
│   ├── matching-engine/        Go
│   ├── position-service/       Go
│   ├── settlement-worker/      Go
│   ├── risk-antifraud-service/ Go
│   ├── referral-service/       Go
│   └── notification-service/   Go
├── packages/
│   ├── proto/                  buf workspace + .proto + generated stubs（v1/）
│   ├── go-sdk/                 Go 內部共用程式庫
│   └── ts-sdk/                 TypeScript 客戶端（給 web/admin）
├── infra/
│   ├── docker-compose.local.yaml   本地 dev 環境（SPEC-017 §5）
│   ├── terraform/                  IaC
│   ├── helm/                       K8s charts
│   ├── argocd/                     GitOps
│   └── runbooks/                   P0 告警 runbook
├── specs/                      ← 本目錄
├── docs/
│   ├── architecture/           架構決策記錄（ADR）
│   ├── postmortems/            事故報告
│   └── diagrams/               圖表
├── design/                     前端設計探索（landing 候選方案）
├── tools/
│   ├── scripts/                bootstrap / migrate / seed / doctor 腳本
│   ├── mocks/                  本地 mock servers（SumSub / 支付）
│   └── load-test/              k6 / Vegeta 壓測
├── .github/
│   └── workflows/              CI（ci.yml / release.yml）
├── go.work
├── pnpm-workspace.yaml
├── turbo.json
├── Makefile
├── buf.yaml
├── buf.gen.yaml
├── .editorconfig
├── .gitignore
├── README.md
├── CLAUDE.md                   ← 不上傳
└── AGENTS.md                   ← 不上傳
```

## 6. 跨服務通訊協議

### 6.1 同步呼叫

**內部服務間**：gRPC + protobuf
- 一致 schema，向後兼容
- 自動產生 stub
- 整合 OTel 追蹤

**對外（玩家）**：REST + JSON
- Cloudflare 友善
- 客戶端容易整合

**WebSocket**：JSON for general，**MessagePack for high-freq**（訂單簿）

**DMM API**：binary protobuf over TLS（呼應 SPEC-009）

### 6.2 異步事件

所有業務事件走 Kafka：

| Topic | Producer | Consumer |
|-------|----------|----------|
| `orders.placed` | api-gateway | matching-engine |
| `engine.events` | matching-engine | wallet, position, ws, risk |
| `wallet.events` | wallet-ledger | reconciliation |
| `market.lifecycle` | market-service | engine, ws, settlement |
| `settlement.events` | settlement-worker | wallet, notification |
| `kyc.events` | kyc-service | user, risk |
| `risk.alerts` | risk-service | compliance, notification |

### 6.3 Schema 管理

- protobuf 集中存於 `packages/proto/`
- buf CLI 管理：lint、break detection
- 變更需 PR review + breaking 標記
- 每個 service consume 固定版本，避免 cascading break

## 7. 資料庫拆分

### 7.1 分庫原則

不嚴格 microservice 拆庫，採用 **schema-per-service** 在同一 PG cluster：

```
postgres://...
  ├─ schema: user
  ├─ schema: wallet     (與 ledger 同 schema 同 transaction)
  ├─ schema: ledger
  ├─ schema: market
  ├─ schema: kyc
  ├─ schema: risk
  ├─ schema: referral
  └─ schema: audit
```

理由：跨服務 ACID transaction 需求（撮合 + 帳本）；DBA 統一管理。

### 7.2 隔離

- 每個 service 使用獨立 DB user，僅有自己 schema 的權限
- 跨 schema 讀取走 view（明確聲明）
- 不允許跨 schema 寫入

### 7.3 高頻寫入隔離

- `audit_logs`、`ledger_entries` 用獨立 partition
- 撮合事件不直接寫 PG，先進 Kafka 再 batch consume
- ClickHouse 處理分析查詢，不打 PG

## 8. Code Style 範例

### 8.1 Go

```go
// services/wallet-ledger-service/internal/ledger/ledger.go

package ledger

import (
    "context"
    "errors"

    "github.com/jackc/pgx/v5"
    "go.opentelemetry.io/otel/trace"
)

var ErrInsufficientFunds = errors.New("insufficient funds")

type Service struct {
    db     *pgx.Conn
    tracer trace.Tracer
}

// PostTransaction posts a balanced double-entry transaction.
// All entries within a transaction must net to zero or it errors.
func (s *Service) PostTransaction(ctx context.Context, txn *Transaction) error {
    ctx, span := s.tracer.Start(ctx, "ledger.PostTransaction")
    defer span.End()

    if err := txn.Validate(); err != nil {
        return err
    }

    tx, err := s.db.Begin(ctx)
    if err != nil {
        return err
    }
    defer tx.Rollback(ctx)

    if err := s.checkIdempotency(ctx, tx, txn.IdempotencyKey); err != nil {
        return err
    }

    if err := s.insertTxn(ctx, tx, txn); err != nil {
        return err
    }
    if err := s.insertEntries(ctx, tx, txn); err != nil {
        return err
    }

    return tx.Commit(ctx)
}
```

### 8.2 React / Next.js 15

```tsx
// apps/web/components/MarketCard.tsx
'use client'

import { Button } from '@cloudflare/kumo'
import { useMarket } from '@/hooks/useMarket'
import { formatPct } from '@/lib/format'
import { OrderBookDepth } from './OrderBookDepth'

type Props = { marketId: string }

export function MarketCard({ marketId }: Props) {
  const { market, depth, lastPrice, isLoading } = useMarket(marketId)

  if (isLoading) return null

  return (
    <article className="rounded-2xl border bg-zinc-900 p-4 transition hover:border-emerald-400">
      <h3 className="text-lg font-semibold">{market?.title}</h3>
      <div className="mt-2 flex items-baseline gap-2">
        <span className="text-2xl font-bold text-emerald-400">{formatPct(lastPrice)}</span>
        <span className="text-xs text-zinc-400">YES</span>
      </div>
      <OrderBookDepth depth={depth} />
      <Button variant="primary" className="mt-4 w-full">下單</Button>
    </article>
  )
}
```

## 9. 命名規範

| 類別 | 規則 | 範例 |
|------|------|------|
| Go package | snake_case 單字 | `ledger`, `engine` |
| Go file | snake_case | `order_book.go` |
| Go type | PascalCase | `OrderBook` |
| Go func | PascalCase（exported）| `MatchOrder` |
| React component | PascalCase, `.tsx` | `MarketCard.tsx` |
| React hook | `use{Name}`, `.ts` | `useMarket.ts` |
| Next.js route segment | kebab-case 目錄 | `app/markets/[id]/page.tsx` |
| Database table | snake_case 複數 | `ledger_entries` |
| Database column | snake_case | `created_at` |
| Kafka topic | dot.delimited | `orders.placed` |
| protobuf | PascalCase | `OrderEvent` |
| K8s resource | kebab-case | `matching-engine` |
| Metric | proph_domain_subject_unit | `proph_engine_match_latency_seconds` |

## 10. 開發者必裝工具

```
gopls            # Go LSP
golangci-lint    # Go lint
gofumpt          # Go format strict
buf              # protobuf 工具
sqlc             # SQL → Go 自動產生
go-migrate       # DB migration
pnpm             # Node 包管理
playwright       # E2E
docker-compose   # local infra
```

## 11. 本地開發環境

`docker-compose.local.yaml` 提供：

- PostgreSQL 16
- Redis 7
- Kafka (single broker, KRaft mode)
- Mock SumSub HTTP server
- Mailhog（攔截 Email）
- Mock 支付 Gateway
- Vault dev mode

`make dev` 啟動全套。

## 12. Tech-Stack 風險評估

| 風險 | 影響 | 緩解 |
|------|------|------|
| Linode 區域故障 | 整站不可用 | DR Singapore, AWS 備援 |
| Cloudflare 故障 | 邊緣 + DNS 失效 | DNS TTL 60s, AWS Route 53 backup |
| SumSub 不可用 | KYC 流程中斷 | 備援 Onfido |
| Kafka 故障 | 撮合事件流中斷 | 本地 WAL + Kafka 重啟可重 replay |
| PostgreSQL primary 故障 | 寫入中斷 | DR + auto-failover |
| Go runtime GC 抖動 | 撮合延遲 | 監控 GC 時間，必要時 rust 重寫 hot path |

## 13. 驗收條件

- [ ] Repo 結構建立，monorepo 工具鏈可用
- [ ] 所有服務有最小 stub 並能 build
- [ ] docker-compose 可一鍵啟動本地環境
- [ ] gRPC + protobuf 跨服務 stub 可生成
- [ ] PG schema 全部 migration 完成
- [ ] CI 跑通所有 service build + lint + test
- [ ] OTel 跨服務 trace 端到端打通

## 14. Open Questions

1. Kafka self-hosted 還是 Confluent Cloud？
2. ClickHouse 還是 TimescaleDB？
3. Argo CD 還是 Flux？
4. Service Mesh 永遠不啟用？還是規模大後再評估 Linkerd？
5. 撮合引擎是否全留 Go？或核心切到 Rust？
