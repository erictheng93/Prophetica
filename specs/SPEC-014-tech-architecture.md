# SPEC-014 — 整體技術架構（Tech Architecture）

> 優先級：跨章節（被多份 SPEC 依賴）
> 版本：v0.2.6 · 最後更新：2026-05-07
>
> **v0.2.6 變更**：§4.1 新增 §4.1.1「目前實作狀態」+ §4.1.1.1「轉 7-binary 之觸發條件」，紀錄 round 7 程式碼驗證結果（當前採 12-binary 拓撲）+ 後續轉換時機之 4 條觸發條件 + 簡述轉換流程；採 12-binary 之 acceptance 補強寫入。SDD 紀律：spec 反映現況。
>
> **v0.2.5 變更**：(1) §3.1 v0.2.0 變更說明補 cross-ref 至 [ADR-0005](../docs/architecture/decisions/ADR-0005-backend-language-rust-over-go.md)（後端 Go → Rust 之 retroactive ADR 落地）；(2) §3.2.1 補 cross-ref 至 [`docs/poc/PRE-FLIGHT-POCS.md §4`](../docs/poc/PRE-FLIGHT-POCS.md)（Cloudflare Pages Edge runtime PoC 詳細 plan）。
>
> **v0.2.4 變更**：(1) **T-1 service 拓撲 advisory**：§4 新增 §4.1「MVP 合併部署 advisory」，建議 MVP / 公開 Beta 階段將 12 個 service binary 合併為 7 個（保留 service crate 結構不變，僅合併 binary）；列拆分時機 + 合併好處（latency budget 寬鬆 / onboarding -40% / CI -30%）+ 代價（scaling 顆粒度）+ Codex PR #1 落地建議。本節為 advisory 不強制；(2) §4 service 表補 cross-ref 至 SPEC-003 §17.2 routing 與 §17.3 thread model 分流。
> **v0.2.3 變更（T-10）**：§3.2 新增 §3.2.1「Cloudflare Pages Edge runtime 限制」，明列 KYC iframe / WebSocket fan-out / Server Actions / fetch 行為 / env binding 五個動工前必須完成 E2E PoC 之場景，含已知限制與退路選項（Node.js runtime / 後端 BFF）。
>
> **v0.2.2 erratum（2026-05-07）**：修正 §3.1 表格首列遺留之「Rust 1.78+ stable」化石為「Rust 1.85+ stable（edition 2024）」，與 v0.2.1 變更說明一致。發現過程：跨檔審視 H-2 版本一致性議題（SPEC-000 v0.2.4 sweep round 3）。
>
> **v0.2.1 變更**：§3.1 補硬性決議 R-1 TLS（rustls 為預設 + rdkafka OpenSSL 例外）、撮合引擎 runtime topology（current_thread + per-market LocalSet + core_affinity）、其餘 service multi-thread runtime、unsafe 全域 forbid + cargo geiger 監測。MSRV 由 1.78 改為 **1.85**（edition 2024）。
>
> **v0.2.0 變更（重大）**：後端語言由 Go 1.22 切換至 **Rust 1.85+ stable**（edition 2024）。所有 12 個 service 改以 Rust 實作，HTTP 層採 axum，gRPC 採 tonic，async runtime tokio，DB 採 sqlx（compile-time checked SQL）。撮合引擎不再保留「未來切 Rust」之選項——直接 Rust first。`go.work` → Cargo workspace；`go-sdk/` → `crates/sdk/`；§14 OQ #5「撮合切 Rust」隨之 Resolved。**本決策之 governance 已於 2026-05-07 由 [ADR-0005](../docs/architecture/decisions/ADR-0005-backend-language-rust-over-go.md) 補做追溯化記錄**（含 driver / 為何全 Rust 而非混搭 / 為何 Rust 而非 C++ / mitigation 細節）。
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
   Player Browser ────►│ ── Next.js 15 SSR/RSC ──► API Gateway (Rust/axum) ►│
                       │                                │                  │
                       │ ── WS connections ──────────► WS Gateway (Rust) ◄─│
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
   │ (Rust single-    │                  │ (持倉管理)        │
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
| 語言 | **Rust 1.85+ stable**（edition 2024） | Go / C++ | 撮合 / ledger 兩大 hot path 全平台受益於 zero-cost abstraction + 無 GC；型別系統（lifetimes / `Send + Sync`）天然鎖死雙簿記 INV 與撮合 single-writer 不變式 |
| Async Runtime | **tokio 1.x** | async-std / smol | 生態系實質單一選項；axum / tonic / sqlx / fred 全在 tokio 之上 |
| HTTP framework | **axum 0.7** | actix-web / warp | tower 中介層生態、與 tonic 共用 service trait |
| gRPC | **tonic 0.11** + prost | grpcio | tokio 原生、prost-build 與 buf 整合佳 |
| DB driver | **sqlx 0.7**（compile-time checked SQL） | tokio-postgres / sea-orm / diesel | 對齊 Go 時代 sqlc 之 compile-time SQL 驗證精神；不引入 ORM |
| Migration | **sqlx-cli** | refinery / dbmate | 與 sqlx 同生態 |
| Redis client | **fred 9.x** | redis-rs | 連線池 + cluster + pubsub 開箱即用 |
| Kafka client | **rdkafka 0.36**（librdkafka 綁定） | rust-rdkafka 純 Rust | librdkafka 為 Confluent 官方 C 庫，性能與相容性最佳 |
| Logging / Tracing | **tracing + tracing-subscriber + opentelemetry** | log + slog | tokio 生態標準，自帶 span / async context propagation |
| Serde | **serde / serde_json / rmp-serde**（MessagePack） | — | 標準 |
| ULID | **`ulid` crate 1.x** | uuid (v7) | 對齊 SPEC-003 §19 OQ #3 |
| TLS | **rustls** (純 Rust) | OpenSSL / native-tls | 與 Cloudflare webpki 生態一致；Distroless image 無系統相依、binary 體積小；fips 需求未來再評。**例外**：`rdkafka` 透過 librdkafka 綁定 OpenSSL，該 crate feature 採 `cmake-build` + `ssl` 走 OpenSSL，其餘 service（sqlx / fred / reqwest / tonic）皆 `rustls-tls` feature |
| 撮合引擎 runtime | **`tokio` current_thread** + per-market `tokio::task::LocalSet` + thread pinning（`core_affinity`） | multi-thread runtime | 撮合熱點避免 work-stealing 抖動；P99 對 tail latency 敏感，single-thread reactor 行為可預測 |
| 其他 service runtime | **`tokio` multi-thread**（預設 worker = CPU 核數） | — | CRUD-style 並發吞吐優先 |
| Unsafe code | **workspace 全域 `#![forbid(unsafe_code)]`** | — | 撮合 SIMD 等少數例外須單檔放行（`#[allow(unsafe_code)]`）+ ADR 記錄 + 4-eye review；CI 跑 `cargo geiger` 做 unsafe 統計門檻警報 |
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

> **為何全 Rust 而非「Rust hot path + Go 其餘」混搭**：
> - **單語言內部 RPC 與 SDK**：12 個 service 之間共用 `crates/sdk/`（observability / errors / proto-gen / ledger client）；混語言要寫兩套 SDK + 兩套 OTel 對接，維運成本高於語言收益。
> - **編譯時間可接受**：sccache + Cargo workspace incremental build，CI 暖快取下單服務 < 90s；Distroless `FROM rust AS builder` + cargo-chef 分層快取使 image build < 5 min。
> - **學習曲線**：CRUD-style service（user / kyc / notification）用 axum + sqlx 程式量與 Go 相當，借助 `?` 運算子甚至更短；borrow checker 對非熱點服務之主要成本為前 2 週適應期。

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

#### 3.2.1 Cloudflare Pages Edge runtime 限制（T-10，動工前 PoC）

**Status (2026-05-08): ❌ live verification blocked before preview deploy.** `apps/web` production `next build` passes, but `@cloudflare/next-on-pages` v1.13.16 failed to generate `.vercel/output/static` on the Windows host while probing `pnpm --version`; a shim retry timed out after 240s. `prophetica-web-poc` Pages project exists, but no preview deployment was produced. Rerun PoC-B on Linux/CI or a fixed Windows toolchain before deciding Edge runtime vs Node.js runtime.

`apps/web` 透過 [`@cloudflare/next-on-pages`](https://github.com/cloudflare/next-on-pages) 部署至 Cloudflare Pages（Workers runtime）。Edge runtime 與標準 Node.js runtime 行為差異會在以下場景踩坑，動工前需完成 E2E PoC 驗證（**詳細 PoC plan 見 [PRE-FLIGHT-POCS §4](../docs/poc/PRE-FLIGHT-POCS.md)**）：

| 場景 | 已知限制 | PoC 驗證項 |
|------|---------|-----------|
| **KYC iframe 嵌入**（SumSub Web SDK）| Edge runtime 對 `Buffer` / `crypto` Node API 有限制；SumSub iframe 內部 postMessage 需 server-side relay 時必須走 Web Crypto API | iframe 載入 → 證件拍攝 → liveness → webhook → 通知前端流程端到端通過 |
| **WebSocket fan-out**（撮合事件即時推播）| Cloudflare Workers 之 WebSocket 為「pair」模式（client ↔ Worker durable object），單一 connection 不可在多個 Worker 之間 share state；訂單簿 WS 推播實際上由 `ws-gateway`（K8s 上的 axum service）處理，**`apps/web` 自身不啟用 Workers WebSocket** | apps/web 連線至 ws-gateway 之 wss:// endpoint 跨 Cloudflare proxy 通過；reconnect / gap detection 行為符合 SPEC-003 §10.3 |
| **Server Actions / RSC**（Next.js 15 App Router）| Edge runtime 不支援部分 Node.js APIs（`fs`、`net`、`tls`、`vm`）；某些 npm package 在 import 時即失敗 | server actions 中呼叫後端 gRPC（透過 fetch + tonic-web 或經 api-gateway REST proxy）通過 build + runtime |
| **fetch 行為差異** | Edge runtime `fetch` 不支援 `keepalive: false`；`AbortController` 行為與 Node 略異 | TanStack Query 之 retry / cancel / abort 在 Edge runtime 下行為一致 |
| **環境變數注入** | Cloudflare Pages 之 env binding 與 Next.js `process.env` 在 Edge runtime 行為不同；secret binding 走 `env.SECRET_NAME` | `.env.local` → `wrangler.toml` 之 [vars] 之 mapping 通過 build；secret 不誤入 client bundle |

> 若 PoC 任一項無法滿足 → 退路選擇：(a) `apps/web` 改部署至 Cloudflare Pages 之 **Node.js runtime**（性能略降但相容性好）；(b) 該功能拆 server-only API endpoint 落在 K8s 上由 `api-gateway` / `bff-service` 處理，`apps/web` 僅做 SSR + 純前端互動。退路選擇需開新 ADR。

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

> **Service 拓撲說明（T-1）**：本表為**目標形態**（DAU > 5,000 後之穩定態），共 12 個 backend services + 2 個 frontend apps。MVP / 公開 Beta 階段建議採 §4.1「合併部署」拓撲降低 onboarding 與 latency 成本，於 DAU > 5,000 或 service 內部複雜度溢出時再拆分為本表形態。

| Service | 語言 | 職責 | 部署規模 |
|---------|------|------|---------|
| `api-gateway` | Rust (axum) | 邊緣 API、認證、路由 | 3+ replicas |
| `ws-gateway` | Rust (axum + tokio-tungstenite) | WebSocket 連線、推播扇出 | 5+ replicas |
| `user-service` | Rust (axum + sqlx) | 註冊、登入、profile、TOTP | 3+ replicas |
| `kyc-service` | Rust | KYC 流程編排、SaaS 整合、webhook | 2+ replicas |
| `wallet-ledger-service` | Rust (tonic + sqlx) | 雙簿記帳本、idempotency | 3+ replicas + leader |
| `market-service` | Rust | 市場 CRUD、Oracle、結算狀態、撮合 routing 決策（SPEC-003 §17.2）| 2+ replicas |
| `matching-engine` | Rust (per-market task on tokio runtime) | 撮合引擎（per-market async task）；MVP 階段熱 / 冷 binary 分流（SPEC-003 §17.3）| DaemonSet, hostNetwork |
| `position-service` | Rust | 持倉聚合 | 3+ replicas |
| `settlement-worker` | Rust | 結算批次處理 | 1 active + standby |
| `risk-antifraud-service` | Rust | 風控規則、行為分析 | 2+ replicas |
| `referral-service` | Rust | 推薦關係、佣金計算 | 1+ replica |
| `notification-service` | Rust | Email / SMS / Push / 站內訊息 | 2+ replicas |
| `web-app` | Next.js 15 (React 19) | 玩家前台 | Cloudflare Pages（`@cloudflare/next-on-pages`） |
| `admin-app` | Next.js 15 (React 19) | 內部後台 | 私網 + mTLS |

### 4.1 MVP 合併部署 advisory（T-1，建議性，非強制）

> **動機**：12 service × 各自 sqlx migrations / OTel instrumentation / Helm chart / Argo CD application = 重複工程量 + onboarding 摩擦；下單熱路徑跨 5+ service hop 累計 P99 也容易超 SPEC-007 §6.4 latency budget。
>
> **建議**：MVP / 公開 Beta 階段先合併部署為 7 個 binary，service crate 結構（`services/*/`）保持本表 12 個分離（**Cargo workspace 不變**）但 build / deploy 為合併 binary。當 DAU > 5,000 或某 service 內部複雜度顯著（例如 KYC SaaS 整合需獨立 scaling）時再拆分。

| MVP binary | 合併之 service crate | 拆分時機 |
|-----------|---------------------|---------|
| `core-api` | `api-gateway` + `user-service` + `market-service` + `position-service` + `referral-service` | DAU > 5k / referral 規模化（簽 B2B 業務 ≥ 10 家）|
| `realtime-gateway` | `ws-gateway` | 一直獨立（WS 連線數 scaling 與 HTTP 不同）|
| `wallet-ledger-service` | 保留獨立 | 帳本服務永遠獨立（attack surface + leader election） |
| `matching-engine-{hot,cold}` | 保留獨立（兩 binary）| SPEC-003 §17.3 已落地分流 |
| `settlement-worker` | 保留獨立（active / standby 模式） | — |
| `kyc-service` | 保留獨立（SaaS 整合 + webhook 安全邊界） | — |
| `notification-and-risk` | `notification-service` + `risk-antifraud-service` | risk 邏輯 ML 化 / 第三方整合複雜化 |

**合併部署的好處**：
- 下單熱路徑減少 4-5 個 gRPC hop → SPEC-007 §6.4 latency budget 更寬裕（風控 + wallet 從跨 service 變 in-process call）
- 12 個 Helm chart → 7 個（onboarding 摩擦 -40%）
- CI 編譯時間 -30%（fewer link units，shared dependencies）

**合併部署的代價**：
- 個別 service 之 scaling 不獨立（如 user-service 無法獨立 scale up）
- attack surface 略增（合併 binary 內部隔離靠 module + workspace lints，不靠 process boundary）

**Codex PR #1 落地建議**：採合併部署。`services/*/` crate 保留本表 12 個結構（lib crate）+ `services/_binaries/{core-api,realtime-gateway,...}/main.rs` 之 7 個 binary crate 引用對應 lib。SPEC-017 §11.2 acceptance check 應反映此拓撲選擇。

> 本 §4.1 為 advisory，**不強制**Codex PR #1 採用；若 Codex 採本表 12-binary 形態實作，需在 PR description 引用 SPEC-007 §6.4 latency budget 並承諾跨 hop 壓測達標。

#### 4.1.1 目前實作狀態（2026-05-07，round 7 程式碼驗證）

> **現況：採 12-binary 拓撲**（每個 `services/*/` 為獨立 binary crate）。本節記錄當前實作選擇 + 後續轉 7-binary 之觸發條件。SDD 紀律：spec 應反映現況。

當前實作選擇：

- 12 個 service 各自獨立 main.rs binary（與 §4 表一致）
- 不存在 `services/_binaries/` 目錄
- 每個 service 透過 `crates/sdk` 之 `service_stub` 模組統一 §6 五要素（healthz / metrics / signal handling / JSON log）
- CI 在單一 `cargo build --workspace` 跑通 16 個 crates（12 services + 2 sdk/proto + 2 mocks）；未設定 GitHub larger runner 時使用 `ubuntu-latest`，若 org/repo 已配置 `ubuntu-latest-4-cores` 才可切換並預期 cold cache < 15 min（SPEC-017 §11.2.10 T-8）

採 12-binary 之 acceptance 補強：

- Codex PR #1 description 必須宣告「採 12-binary，下單熱路徑跨 5+ hop 將於 staging 完成 SPEC-007 §6.4 latency budget 壓測」
- 壓測結果若超 budget → 觸發 §4.1.1.1 轉 7-binary 路徑

##### 4.1.1.1 轉 7-binary 之觸發條件（將來）

下列任一成立時，啟動轉 7-binary 拓撲之 PR：

1. **Latency budget 違反**：staging 壓測下單熱路徑 P99 > 30ms（SPEC-007 §6.4 端到端 SLO）持續 7 天以上
2. **CI cold cache > 25 min**：4-cores runner 編譯時間超預期，需減少 link unit 數
3. **DAU < 5,000 且 service hop 占 P99 > 50%**：成本不效益，合併方為更佳 ROI
4. **Compliance Officer / Tech Lead 評估**：跨 service log / trace 對 Compliance 個案調查造成顯著摩擦

轉換流程（簡述，詳細待轉換時開新 ADR）：

- 建立 `services/_binaries/{core-api, realtime-gateway, notification-and-risk}/main.rs` 對應本節合併方案
- 12 個 service crate 之 `Cargo.toml` 改為 lib-only（`[lib]` 取代 `[[bin]]`）
- 對應 7 個 binary crate 引用合併之 lib crates
- Helm chart / Argo CD application 由 12 個合併為 7 個
- CI ci.yml 之 deploy step 對齊新拓撲

## 5. Repository 結構（Monorepo）

採用 **Turborepo + pnpm workspaces** + **Cargo workspace（root `Cargo.toml`）**

> 此處為**規範來源（normative）**；SPEC-017 §2 為 bootstrap 後的實際 file tree，必須與本節一致。

```
prophetica/
├── apps/
│   ├── web/                    Next.js 15 玩家前台（React + Kumo）
│   ├── admin/                  內部後台
│   └── liff/                   LINE LIFF mini app
├── services/
│   ├── api-gateway/            Rust crate（axum）
│   ├── ws-gateway/             Rust crate
│   ├── user-service/           Rust crate
│   ├── kyc-service/            Rust crate
│   ├── wallet-ledger-service/  Rust crate
│   ├── market-service/         Rust crate
│   ├── matching-engine/        Rust crate
│   ├── position-service/       Rust crate
│   ├── settlement-worker/      Rust crate
│   ├── risk-antifraud-service/ Rust crate
│   ├── referral-service/       Rust crate
│   └── notification-service/   Rust crate
├── crates/
│   ├── sdk/                    Rust 內部共用 lib（observability / errors / config / ledger client）
│   └── proto/                  prost / tonic 產出之 Rust stub（與 packages/proto 來源同步）
├── packages/
│   ├── proto/                  buf workspace + .proto + generated stubs（v1/）— 包含 ts/ 與 rust/ 兩個輸出
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
├── Cargo.toml                  ← workspace root（[workspace] members = ["services/*", "crates/*"]）
├── Cargo.lock
├── rust-toolchain.toml         ← 鎖定 Rust 1.85+ stable（edition 2024）
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

### 8.1 Rust

```rust
// services/wallet-ledger-service/src/ledger.rs

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use tracing::instrument;

use crate::domain::{LedgerTxn, IdempotencyKey};

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("idempotency key already used")]
    DuplicateIdempotencyKey,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("entries do not balance to zero")]
    Unbalanced,
}

pub struct LedgerService {
    pool: PgPool,
}

impl LedgerService {
    /// Posts a balanced double-entry transaction.
    /// All entries within a transaction must net to zero or it errors.
    #[instrument(skip(self, txn), fields(idempotency_key = %txn.idempotency_key))]
    pub async fn post_transaction(&self, txn: LedgerTxn) -> Result<(), LedgerError> {
        txn.validate().map_err(|_| LedgerError::Unbalanced)?;

        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        self.check_idempotency(&mut tx, &txn.idempotency_key).await?;
        self.insert_txn(&mut tx, &txn).await?;
        self.insert_entries(&mut tx, &txn).await?;

        tx.commit().await?;
        Ok(())
    }
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
| Rust crate | kebab-case | `wallet-ledger-service`, `sdk` |
| Rust module | snake_case | `ledger`, `engine`, `order_book` |
| Rust file | snake_case | `order_book.rs` |
| Rust type | PascalCase | `OrderBook`, `LedgerService` |
| Rust function / method | snake_case | `match_order`, `post_transaction` |
| Rust constant | SCREAMING_SNAKE_CASE | `MAX_PRICE_PTS` |
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
rustup           # Rust toolchain manager（rust-toolchain.toml 自動鎖定 channel/version）
rust-analyzer    # Rust LSP
clippy           # Rust lint（rustup component add clippy）
rustfmt          # Rust format（rustup component add rustfmt）
sccache          # 編譯快取（極大幅縮短 incremental build）
cargo-nextest    # 平行測試 runner（取代 cargo test，更快、更好的 UI）
cargo-deny       # 依賴審查（授權 / advisory / 重複版本）
cargo-chef       # Docker layer caching for Cargo
sqlx-cli         # DB migration + offline query metadata
buf              # protobuf 工具
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
| Rust 編譯時間 | 開發迴圈 / CI 慢 | sccache 共享快取、cargo-chef 分層 Docker、開發者本機用 cargo-watch + 增量編譯；CI 暖快取下單服務 < 90s |
| tokio runtime 阻塞 | 撮合延遲尖刺 | 撮合熱路徑禁止呼叫阻塞 syscall（DB / 檔案）；用 `spawn_blocking` 隔離；CI 加 lint rule（tokio-console + 自訂 clippy lint） |
| 團隊 Rust 經驗曲線 | 早期 PR 速度慢 | 前 4 週 pair programming + code review 嚴格把關 unwrap / unsafe；定期內部讀書會（async-book / The Rustonomicon hot path 章節） |

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
5. ~~撮合引擎是否全留 Go？或核心切到 Rust？~~ ✅ **Resolved 2026-05-05（v0.2.0）**：全平台採 Rust，本議題隨之吸收。
