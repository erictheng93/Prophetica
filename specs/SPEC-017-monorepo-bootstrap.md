# SPEC-017 — Monorepo Bootstrap & Local Dev Environment

> 優先級：**P0（必須在第一行業務 code 之前完成）**
> 版本：v0.2.0 · 最後更新：2026-05-05
> 依賴：SPEC-014（架構定義）
> 被依賴：所有 SPEC（實作載體）

> **v0.2.0 變更（重大）**：跟隨 SPEC-014 v0.2.0 後端切 Rust，本 SPEC 大幅改寫：(1) §2 service 全標 Rust crate；新增 `crates/sdk/`、`crates/proto/`，移除 `packages/go-sdk/`；mocks 改 Rust；(2) §3 toolchain：Go / sqlc / golangci-lint / go-migrate / gofumpt 全替換為 Rust 1.78+ / clippy / rustfmt / sqlx / sqlx-cli + 新增 sccache / cargo-nextest / cargo-chef / cargo-deny；(3) §4.2 `go.work` → 根 `Cargo.toml` workspace；(4) §4.4 buf.gen.yaml Go plugin → prost/tonic Rust plugin；(5) §4.5 Makefile go 命令 → cargo；(6) §6.1 Rust axum 範例取代原 Echo 範例；(7) §8 CI 改 dtolnay/rust-toolchain action + sccache cache；(8) §11 acceptance / §12 OQ 內文同步。

---

## 1. Objective

定義 prophetica 從「git clone 到 `make dev` 啟動本地全套服務」的最小可重現流程。本 SPEC 是「可實施規格」中**唯一不對應某個業務 service** 的 meta-SPEC，目的是讓 Codex / 新進工程師在第一個 PR 之前有明確的 bootstrap 步驟，避免每位實作者各自詮釋 SPEC-014 §5 的目錄結構。

### 1.1 In-scope

- Repository 骨架（Turborepo + pnpm workspaces + Cargo workspace）
- Docker Compose 本地開發環境
- Makefile / pnpm script 統一入口
- 各 service / app 之最小 hello-world stub
- CI bootstrap（GitHub Actions 最小可運行 workflow）
- 開發者首次設定流程

### 1.2 Out-of-scope

- 業務邏輯（由各 SPEC 定義）
- Production K8s / Helm / Terraform（由 SPEC-013 / SPEC-008 定義）
- protobuf message 內容（見 §4.3，schema 細節在 `packages/proto/v1/`）

---

## 2. 目錄結構（與 SPEC-014 §5 一致）

> SPEC-014 §5 為**規範來源**，本節為**實際 bootstrap 後的 file tree 對應**。任一不一致以 SPEC-014 為準並更新本節。

```
prophetica/
├── apps/
│   ├── web/                    Next.js 15 (React 19) 玩家前台
│   ├── admin/                  Next.js 15 內部後台
│   └── liff/                   LINE LIFF mini app
├── services/                   Rust workspace members（每個 service 為獨立 crate）
│   ├── api-gateway/            Rust crate（axum）
│   ├── ws-gateway/             Rust crate（axum + tokio-tungstenite）
│   ├── user-service/           Rust crate
│   ├── kyc-service/            Rust crate
│   ├── wallet-ledger-service/  Rust crate（tonic + sqlx）
│   ├── market-service/         Rust crate
│   ├── matching-engine/        Rust crate（DaemonSet, hostNetwork in prod）
│   ├── position-service/       Rust crate
│   ├── settlement-worker/      Rust crate
│   ├── risk-antifraud-service/ Rust crate
│   ├── referral-service/       Rust crate
│   └── notification-service/   Rust crate
├── crates/                     Rust workspace members（共用 lib crate）
│   ├── sdk/                    內部共用 lib（observability / errors / config / ledger client）
│   └── proto/                  prost / tonic 產出之 Rust stub crate（src/ 由 buf generate 寫入）
├── packages/
│   ├── proto/                  buf workspace + .proto + generated stubs（gen/ts/, gen/rust→symlink/copy 至 crates/proto/src/）
│   │   └── v1/                 ← protobuf v1 集中地
│   └── ts-sdk/                 TypeScript 客戶端（給 web/admin）
├── infra/
│   ├── docker-compose.local.yaml   本地 dev 環境
│   ├── terraform/                  IaC（暫空）
│   ├── helm/                       K8s charts（暫空）
│   ├── argocd/                     GitOps（暫空）
│   └── runbooks/                   P0 告警 runbook
├── specs/                          ← SPEC v0.1.x
├── docs/
│   ├── architecture/decisions/     ADR
│   ├── postmortems/                事故報告（暫空）
│   └── diagrams/                   圖表（暫空）
├── design/                         前端設計探索（landing 候選方案）
├── tools/
│   ├── scripts/                    bootstrap / migrate / seed / doctor 腳本（被 §4.5 Makefile 引用）
│   ├── mocks/                      Rust binaries（cargo workspace members）
│   │   ├── sumsub/                 SumSub KYC webhook 模擬（§5）
│   │   └── payment/                支付 Gateway webhook 模擬（§5）
│   └── load-test/                  k6 / Vegeta 壓測（暫空）
├── .github/
│   └── workflows/
│       ├── ci.yml                  PR / main 觸發
│       └── release.yml             tag 觸發
├── Cargo.toml                      ← workspace root
├── Cargo.lock
├── rust-toolchain.toml             ← 鎖定 Rust 1.78+ stable
├── .cargo/config.toml              ← sccache wrapper、profile 設定
├── pnpm-workspace.yaml
├── turbo.json
├── Makefile
├── buf.yaml
├── buf.gen.yaml
├── .gitignore
├── .editorconfig
├── README.md
└── CLAUDE.md                       ← gitignored
```

---

## 3. 工具鏈版本鎖定

> 工具版本不一致是 monorepo 最常見的 onboarding 摩擦。以下為**強制版本**，由 `Makefile` 之 `make doctor` 檢查。

| 工具 | 版本 | 鎖定方式 |
|------|------|---------|
| Node.js | 20 LTS | `.nvmrc` |
| pnpm | 9.x | `package.json#packageManager` |
| Next.js | 15.x | `apps/*/package.json` peer 鎖 |
| React / React-DOM | 19.x | 同上 |
| `@cloudflare/kumo` | latest stable | 同上；私有 registry 須在 `.npmrc` 指向 Cloudflare（見 §3.2） |
| `@phosphor-icons/react` | 與 Kumo peer 對齊 | 同上 |
| Rust | 1.78+ stable | `rust-toolchain.toml`（`channel = "1.78"`，鎖 components: rustfmt / clippy / rust-analyzer）|
| Docker | 24+ | `make doctor` 檢查 |
| Docker Compose | v2（plugin 形式）| 同上 |
| buf | 1.30+ | `Makefile` install target |
| sqlx-cli | 0.7+ | `cargo install sqlx-cli --no-default-features --features postgres` 由 `make doctor` 確認 |
| sccache | 0.7+ | 同上；`.cargo/config.toml` 設 `rustc-wrapper = "sccache"` |
| cargo-nextest | 0.9+ | 同上；`make test` 預設走 `cargo nextest run` |
| cargo-chef | 0.1+ | Dockerfile 用，本機可選 |
| cargo-deny | 0.14+ | CI 跑 `cargo deny check`（許可、advisory、bans）|
| make | 任意（Linux）/ Make for Windows / WSL2 | 文件提示 |

### 3.2 `@cloudflare/kumo` registry 配置

Kumo 由 Cloudflare 私有 registry 發佈（非 npmjs.com），repo 根目錄 `.npmrc` 須包含：

```
@cloudflare:registry=https://npm.cloudflare.com
//npm.cloudflare.com/:_authToken=${KUMO_NPM_TOKEN}
```

`KUMO_NPM_TOKEN` 由 Vault 注入；CI 從 GitHub Actions secret `KUMO_NPM_TOKEN` 取得。本機開發者首次 `make bootstrap` 時 `make doctor` 須檢查此環境變數存在。

### 3.3 Windows 開發者特別說明

prophetica 主要開發機之一為 Windows + PowerShell。為避免每位開發者自行設定：

- 所有 `make` 目標必須能在 **WSL2 Ubuntu 22.04** 上跑通
- `docker-compose.local.yaml` 不依賴 host network mode（Windows Docker Desktop 不支援）
- 路徑分隔符不用反斜線（一律 `/`）

---

## 4. Repository 根目錄關鍵檔案

### 4.1 `pnpm-workspace.yaml`

```yaml
packages:
  - "apps/*"
  - "packages/ts-sdk"
  - "packages/proto"   # buf workspace 也需 pnpm 解析
```

### 4.2 `Cargo.toml`（workspace root）

```toml
[workspace]
resolver = "2"
members = [
    "services/api-gateway",
    "services/ws-gateway",
    "services/user-service",
    "services/kyc-service",
    "services/wallet-ledger-service",
    "services/market-service",
    "services/matching-engine",
    "services/position-service",
    "services/settlement-worker",
    "services/risk-antifraud-service",
    "services/referral-service",
    "services/notification-service",
    "crates/sdk",
    "crates/proto",
    "tools/mocks/sumsub",
    "tools/mocks/payment",
]

[workspace.package]
version    = "0.1.0"
edition    = "2021"
rust-version = "1.78"
license    = "Proprietary"

[workspace.dependencies]
tokio        = { version = "1", features = ["full"] }
axum         = "0.7"
tonic        = "0.11"
prost        = "0.12"
sqlx         = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros"] }
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
tracing      = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
opentelemetry = "0.22"
opentelemetry-otlp = "0.15"
fred         = "9"
rdkafka      = "0.36"
ulid         = "1"
thiserror    = "1"
anyhow       = "1"
dotenvy      = "0.15"

[profile.release]
lto         = "thin"
codegen-units = 1
panic       = "abort"
strip       = "symbols"
```

### 4.2.1 `rust-toolchain.toml`

```toml
[toolchain]
channel    = "1.78"
components = ["rustfmt", "clippy", "rust-analyzer"]
profile    = "minimal"
```

### 4.2.2 `.cargo/config.toml`

```toml
[build]
rustc-wrapper = "sccache"

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### 4.3 `turbo.json`

```json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": { "dependsOn": ["^build"], "outputs": [".next/**", "dist/**"] },
    "lint":  { "outputs": [] },
    "test":  { "dependsOn": ["build"], "outputs": ["coverage/**"] },
    "dev":   { "cache": false, "persistent": true }
  }
}
```

### 4.4 `buf.yaml` / `buf.gen.yaml`

```yaml
# buf.yaml
version: v2
modules:
  - path: packages/proto
lint:
  use: [DEFAULT]
breaking:
  use: [FILE]
```

```yaml
# buf.gen.yaml
version: v2
plugins:
  # Rust：prost (messages) + tonic (gRPC services)
  # 採 protoc-gen-prost / protoc-gen-tonic 之 buf-managed 版本
  - remote: buf.build/community/neoeinstein-prost
    out: crates/proto/src/gen
    opt: [bytes=., compile_well_known_types]
  - remote: buf.build/community/neoeinstein-tonic
    out: crates/proto/src/gen
    opt: [no_include, compile_well_known_types]
  # TypeScript（前端 / ts-sdk 共用）
  - remote: buf.build/bufbuild/es
    out: packages/proto/gen/ts
    opt: target=ts
```

> 產出後，`crates/proto/src/lib.rs` 以 `pub mod proph { include!("gen/proph.v1.rs"); }` 形式 expose；service 透過 `proto::proph::v1::*` 引用。SPEC-017 §12 OQ #1 已 Resolved 為 commit gen/，本章 `crates/proto/src/gen/` 同樣 commit。

### 4.5 `Makefile`（最低 target）

```makefile
.PHONY: doctor bootstrap dev down test lint fmt proto migrate clean

doctor:
	@./tools/scripts/check-versions.sh

bootstrap:
	pnpm install --frozen-lockfile
	cargo fetch
	$(MAKE) proto
	cargo build --workspace --locked

dev:
	docker compose -f infra/docker-compose.local.yaml up -d
	pnpm turbo run dev --parallel

down:
	docker compose -f infra/docker-compose.local.yaml down -v

test:
	pnpm turbo run test
	cargo nextest run --workspace --locked

lint:
	pnpm turbo run lint
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo fmt --all -- --check

fmt:
	cargo fmt --all
	pnpm turbo run format

proto:
	cd packages/proto && buf generate

migrate:
	sqlx migrate run --source services/wallet-ledger-service/migrations
	# 其他 service 之 migrations 由 tools/scripts/migrate.sh 串接

clean:
	rm -rf node_modules apps/*/.next packages/proto/gen crates/proto/src/gen
	cargo clean
```

---

## 5. `infra/docker-compose.local.yaml` 服務清單

> 目標：**`make dev` 之後 60 秒內全部 healthy**。

| 服務 | Image | 預設 Port | 用途 |
|------|-------|----------|------|
| `postgres` | `postgres:16-alpine` | 5432 | 主 DB（含 ledger schema） |
| `redis` | `redis:7-alpine` | 6379 | Cache / Rate limit / Idempotency |
| `kafka` | `confluentinc/cp-kafka:7.6.0`（KRaft 模式單 broker） | 9092 | 事件 bus |
| `vault` | `hashicorp/vault:1.15`（dev mode） | 8200 | 本地 secrets |
| `mailhog` | `mailhog/mailhog:latest` | 1025 / 8025 | Email 攔截 |
| `mock-sumsub` | 自建 stub（`tools/mocks/sumsub`） | 8081 | KYC webhook 模擬 |
| `mock-payment` | 自建 stub（`tools/mocks/payment`） | 8082 | 通路 webhook 模擬 |
| `prometheus` | `prom/prometheus:latest` | 9090 | Metrics scrape |
| `grafana` | `grafana/grafana:latest` | 3000（dev 改 3001 避撞 web）| Dashboard |
| `tempo` | `grafana/tempo:latest` | 4317 (OTLP) | Tracing |
| `loki` | `grafana/loki:latest` | 3100 | Logs |

### 5.0 本地不啟用之元件

下列 SPEC-014 列出的元件**本地 dev 不啟用**，避免拖慢 `make dev`；服務在本地以 in-memory / 降級模式運行，於 staging 起方啟用。任何 service 之單元測試**不得**依賴下列元件可用：

| 元件 | SPEC-014 出處 | 本地替代 | 啟用環境 |
|------|--------------|---------|---------|
| TimescaleDB | §3.1 | PostgreSQL（同 image，不啟用 timescaledb extension）| staging+ |
| Meilisearch | §3.1 | 市場查詢走 PG ILIKE | staging+ |
| ClickHouse | §7.3 | 分析查詢暫無 | staging+ |

### 5.1 Healthcheck 範例

每個服務必須有 healthcheck，使 `docker compose up -d` 等到 `healthy` 才返回：

```yaml
postgres:
  image: postgres:16-alpine
  environment:
    POSTGRES_USER: proph
    POSTGRES_PASSWORD: dev
    POSTGRES_DB: prophetica
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U proph -d prophetica"]
    interval: 3s
    timeout: 3s
    retries: 10

grafana:
  image: grafana/grafana:latest
  ports:
    - "3001:3000"   # host:container — 避撞 apps/web Next.js dev server (3000)
  environment:
    GF_AUTH_ANONYMOUS_ENABLED: "true"
```

### 5.2 種子資料（Seed）

`make dev` 之後自動跑 `tools/scripts/seed.sh`：

- 建立 PG schema（執行 SPEC-002 / SPEC-015 migrations）
- 載入測試帳號 3 個：`alice`（kyc_tier=`TIER_2`）、`bob`（kyc_tier=`TIER_1`）、`dmm-001`（DMM 法人帳戶，kyc_tier=`TIER_2`）；enum 字面以 SPEC-005 §2.1 canonical identifier 為準
- 載入 1 個測試市場 `mkt_dev_demo`
- 不建立 `PLATFORM_SYSTEM_USER_IDS` 之 user 紀錄（呼應 SPEC-003 §12.1）

---

## 6. 各 service 最小 stub

每個 service 必須提供以下五要素，方能列為「bootstrap complete」：

| 要素 | 內容 |
|------|------|
| `src/main.rs` + `Cargo.toml` | tokio runtime 啟動 + signal handling + graceful shutdown |
| `/healthz/live` | liveness（process up）|
| `/healthz/ready` | readiness（DB / Redis / Kafka 可達）|
| `/metrics` | Prometheus scrape（`axum-prometheus` or `metrics-exporter-prometheus`）|
| 結構化 JSON log | stdout（tracing-subscriber JSON formatter），符合 SPEC-007 §4.1 格式 |

### 6.1 Rust service 範例（`services/api-gateway/src/main.rs`）

```rust
use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde_json::json;
use tokio::signal;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // structured JSON log（SPEC-007 §4.1）
    fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = Router::new()
        .route("/healthz/live",  get(|| async { Json(json!({"status": "alive"})) }))
        .route("/healthz/ready", get(|| async { Json(json!({"status": "ready"})) }));
    // /metrics 由 axum-prometheus middleware 注入；省略

    let addr: SocketAddr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "api-gateway started");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.ok(); };
    #[cfg(unix)]
    let term = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .ok().and_then(|mut s| futures::executor::block_on(async { s.recv().await; Some(()) }));
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
```

### 6.2 Next.js app 範例（`apps/web/`）

- `app/page.tsx`：顯示 "prophetica web — bootstrapping"
- `app/api/health/route.ts`：回 `{status: "ok"}`
- `next.config.mjs`：含 `@cloudflare/next-on-pages` 設定（呼應 ADR-0004）

### 6.3 admin / liff app stub 條件

| App | 最小 stub 要素 |
|-----|---------------|
| `apps/admin/` | `app/page.tsx` 顯示登入佔位；`app/api/health/route.ts`；middleware 設置「僅內網 / mTLS placeholder」註記；不部署至公開 Cloudflare Pages |
| `apps/liff/` | `app/page.tsx` 載入 LIFF SDK 並 `liff.init({ liffId: process.env.NEXT_PUBLIC_LIFF_ID })`；`app/api/health/route.ts`；`NEXT_PUBLIC_LIFF_ID` 缺值時 fail-fast |

三個 Next.js app（web / admin / liff）共用 `packages/ts-sdk` 與 Kumo 元件，由 Turborepo `^build` 依賴拓撲連動。

---

## 7. 環境變數與配置

### 7.1 `.env.example`（repo 提供）

```
# Database
PROPH_DB_DSN=postgres://proph:dev@localhost:5432/prophetica?sslmode=disable

# Redis
PROPH_REDIS_URL=redis://localhost:6379/0

# Kafka
PROPH_KAFKA_BROKERS=localhost:9092

# Vault
PROPH_VAULT_ADDR=http://localhost:8200
PROPH_VAULT_TOKEN=dev-root-token

# Observability
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=api-gateway

# Mock providers
PROPH_KYC_PROVIDER_URL=http://localhost:8081
PROPH_PAYMENT_PROVIDER_URL=http://localhost:8082
```

### 7.2 配置注入規則

- 開發環境：`.env` 透過 docker-compose `env_file` 注入；直接 `cargo run` 之開發者由 `dotenvy::dotenv()` 在 `main()` 起始載入
- 生產環境：Vault sidecar agent（呼應 SPEC-010 §3.3）
- 禁區：**禁止**將 secret 寫入 `ConfigMap` 或 image
- 禁區：**禁止**在 Rust code 中 hardcode default secret（`std::env::var(...)` 沒值就 `anyhow::bail!` fail-fast，禁止 `.unwrap_or("dev-secret")` 之類 fallback）

---

## 8. CI Bootstrap（`.github/workflows/ci.yml`）

> 完整 CI 設計在 SPEC-013，本節僅定義 bootstrap 階段必須通過的最小 gate。

```yaml
name: ci
on:
  pull_request:
  push:
    branches: [main]

jobs:
  lint-and-test:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version-file: '.nvmrc'
      - uses: pnpm/action-setup@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: '1.78'
          components: rustfmt,clippy
      - uses: Swatinem/rust-cache@v2     # cargo registry / target cache
      - uses: mozilla-actions/sccache-action@v0.0.4
      - uses: bufbuild/buf-setup-action@v1
      - name: install cargo-nextest
        uses: taiki-e/install-action@nextest
      - name: install sqlx-cli + cargo-deny
        run: |
          cargo install sqlx-cli --no-default-features --features postgres --locked
          cargo install cargo-deny --locked

      - name: pnpm install
        run: pnpm install --frozen-lockfile
      - name: buf generate
        run: cd packages/proto && buf generate
      - name: buf lint
        run: cd packages/proto && buf lint
      - name: buf breaking (vs main)
        run: cd packages/proto && buf breaking --against '.git#branch=main'
      - name: verify generated stubs in sync
        run: git diff --exit-code crates/proto/src/gen packages/proto/gen/ts

      - name: lint
        run: make lint
      - name: test
        run: make test
      - name: cargo deny
        run: cargo deny check

  sentinel-scan:
    runs-on: ubuntu-22.04
    needs: lint-and-test
    steps:
      - uses: actions/checkout@v4
      - name: ADR-0001 §6 forbidden words
        run: |
          if grep -rn -E 'auto_quote|market_balancer|house_book|probability_symmetry|auto_hedge' \
               --include='*.rs' services/ crates/ ; then
            echo "::error::ADR-0001 §6 forbidden words detected"; exit 1; fi
      - name: production binary must not depend on tools/mocks
        run: |
          if grep -rn 'tools/mocks' services/ crates/sdk/ ; then
            echo "::error::tools/mocks referenced from production code"; exit 1; fi
```

> `release.yml`（tag 觸發、build & push image、產 SBOM、ArgoCD sync）之完整內容定義於 [SPEC-013 § 3](./SPEC-013-cicd-release.md)；本 SPEC 不重複規範。Bootstrap 階段僅需在 `.github/workflows/release.yml` 放置一個 `on: push: tags: ['v*']` 的 placeholder workflow（內容為 `echo "release pipeline owned by SPEC-013"`），讓 tag push 不會 404。

### 8.1 必過 gate

| Gate | 阻斷條件 |
|------|---------|
| `pnpm install --frozen-lockfile` | pnpm-lock.yaml 不一致 |
| `cargo build --workspace --locked` | Cargo.lock 不一致 / 編譯錯誤 |
| `buf lint` | proto 違反 lint |
| `buf breaking` | 對 main 有 breaking change（未經 review）|
| `verify generated stubs in sync` | .proto 改了但 commit 內 gen/ 未同步 regenerate |
| `make lint` | clippy `-D warnings` / rustfmt / eslint / prettier 違反 |
| `make test` | 任一測試 fail（cargo nextest + pnpm turbo test）|
| `cargo deny check` | 依賴授權 / advisory / banned crate 違反 |
| `sentinel-scan` ADR-0001 | 禁字命中 |
| `sentinel-scan` mocks isolation | production code 引用 `tools/mocks` |

---

## 9. 開發者首次設定流程

新進工程師 / Codex 第一次 clone 後執行：

```bash
# <tech-co-org> 為 ADR-0002 Tech Co. 之 GitHub organization；最終命名以執照地律師
# 與工商登記決定後填入，過渡期可用個人 fork
git clone https://github.com/<tech-co-org>/prophetica.git
cd prophetica

# 1. 檢查工具版本
make doctor

# 2. 安裝依賴 + 產生 proto stub
make bootstrap

# 3. 啟動本地服務
make dev

# 4. 跑 migration
make migrate

# 5. 確認所有服務 healthy
curl localhost:8080/healthz/ready    # api-gateway
curl localhost:3000                   # next.js web

# 6. 跑測試確認綠
make test
```

預計**首次完整 bootstrap 時間 < 10 分鐘**（不含 docker image pull）。

---

## 10. 與其他 SPEC 的銜接

| SPEC | 銜接點 |
|------|--------|
| SPEC-014 §5 | 目錄結構唯一定義來源；本 SPEC 之 §2 必與其一致 |
| SPEC-013 § 3 | CI 完整 pipeline；本 SPEC §8 為其最小 bootstrap 子集 |
| SPEC-010 §3.4 | 依賴掃描在 bootstrap 後第一個 PR 啟用 |
| SPEC-007 §3 / §4 | 各服務 stub 必輸出符合命名規範的 metric 與結構化 log |
| SPEC-002 §4.3 | migration 工具（sqlx-cli）的腳本位置與 `migrations/` 目錄慣例 |
| SPEC-014 §6.3 | protobuf schema 治理（buf lint / breaking）；本 SPEC §4.4 / §8 將 schema gate 落地到 buf workspace 與 CI |
| ADR-0001 §6 | sentinel scan 在 CI 落地 |
| ADR-0004 | Next.js 15 + `@cloudflare/kumo` 在 `apps/web/` 落地 |

---

## 11. 驗收條件

- [ ] §2 file tree 全部建立且通過 `make doctor`
- [ ] `make bootstrap` 在乾淨環境（無 cache）完成 < 10 分鐘
- [ ] `make dev` 之後所有 docker-compose 服務 healthy < 60 秒
- [ ] `make test` 在 bootstrap 完成後立即全綠（最小 stub 也算）
- [ ] CI `.github/workflows/ci.yml` 在第一個 PR 通過 lint / test / sentinel-scan 三 gate
- [ ] 任一新進工程師依 §9 流程能在 30 分鐘內首次 bootstrap 成功
- [ ] `buf generate` 為每個 .proto 產出 Rust（`crates/proto/src/gen/`）+ TS（`packages/proto/gen/ts/`）stub 並 commit 進 git

---

## 12. Open Questions

1. ~~`packages/proto/gen/` 是否 commit 進 git？~~ ✅ **Resolved 2026-05-05**：**commit 進 git**（含 Rust + TS 雙語言）。理由：(a) 跨機器（macOS / Windows / Linux CI）import path 一致，rust-analyzer / IDE 立即可解析；(b) 新人首次 clone 即可 `cargo build`，不必先跑 `buf generate`；(c) PR diff 會明確顯示 schema 變更對 stub 的影響，作為 schema 評審輔證；(d) CI 跑 `git diff --exit-code crates/proto/src/gen packages/proto/gen/ts` 確保 .proto 與 gen/ 同步（不一致則 CI fail）。落地動作：兩個目錄皆不加入 `.gitignore`；新增 `make proto` target 規範 regenerate 流程。注意：Rust 慣例為 build.rs runtime codegen，本決策刻意偏離以對齊 monorepo 全語言一致性策略。
2. ~~`.env` 配置注入採 `direnv` vs `docker-compose env_file` vs `dotenv-cli`？~~ ✅ **Resolved 2026-05-05**：採 **`docker-compose env_file`** 為主，理由與落地：
   - direnv 在 PowerShell / Windows 原生支援差，本團隊 macOS + Windows 雙平台 → 排除
   - dotenv-cli 多一層 npm wrapper，Rust service 仍要自己讀 env，價值有限 → 排除
   - **`docker-compose env_file: .env`** 為事實標準、跨平台、與 §5 docker stack 天然契合 → 採用
   - 直接跑 `cargo run -p <service>` 之開發者：Rust side 統一用 [`dotenvy`](https://docs.rs/dotenvy)（`dotenvy::dotenv().ok();` 於 main 起始）；Next.js side 走內建 `.env.local`（apps/web/）
   - 範本檔：repo 根 `.env.example`（已知 key + 註解 + 安全預設值），`make bootstrap` 首次執行時 copy 為 `.env` 若不存在
   - **嚴格禁止** 將 `.env` commit 進 git（已加入 `.gitignore`）；secrets 載入 prod 走 SPEC-010 §6 secret manager
3. ~~mock servers（`tools/mocks/sumsub` / `payment`）是否獨立 repo 或同 monorepo？~~ ✅ **Resolved 2026-05-05**：**同 monorepo**（路徑 `tools/mocks/{sumsub,payment}/`）。理由：
   - **版本同步零成本**：mock 必須與 SPEC-005 KYC webhook payload schema、SPEC-016 §5.5 6 家通路 webhook 對映表、`packages/proto/v1/kyc.proto` / `payment.proto` 同步演化。獨立 repo 會造成 schema drift，CI 無法在同一 PR 跨 repo 驗證。
   - **新人 onboarding 簡化**：`make dev` 一次拉起含 mocks 的完整 stack，不必額外 clone repo / 設 token。
   - **CI 共享 cache**：Rust workspace 與 protobuf stub 已在 monorepo 編譯，mocks 共用 sccache / cargo target cache。
   - **嚴格隔離**：mocks 僅在 `make dev` / `make test` 啟用；`tools/mocks/` 透過 `.dockerignore` 與 Cargo feature flag（`[features] mock = []`，僅 mocks crate 自身依賴）確保**絕無**進入 production binary 之路徑；§8 CI 加 grep gate：`services/` 與 `crates/sdk/` 之 Cargo.toml 不得依賴 `tools/mocks/*` crate。
4. Windows 主機之 host networking 限制如何處理 hostNetwork 撮合引擎部署？本地 dev 不啟用 hostNetwork（僅 prod K8s 啟用），由 SPEC-008 §3 監督。
5. ~~CI matrix（macOS / Linux / Windows）是否啟用？~~ ✅ **Resolved 2026-05-05**：採 **「Linux 主 gate + macOS nightly + 不跑 Windows runner」** 三層策略：
   - **PR gate（每次 push）**：`ubuntu-latest` only — lint / test / build / sentinel-scan / `buf generate --check`。生產環境為 Linux，主 gate 必須與 prod 一致。
   - **Nightly（cron）**：`macos-latest` 跑全套 unit test + `make doctor`。理由：團隊有 macOS 開發者，需早期捕捉路徑分隔符 / 檔名大小寫敏感性 / Apple Silicon (aarch64) 之 Rust cross-compile 差異（rdkafka 之 librdkafka native 綁定常見差異點）。Nightly 失敗發 Slack alert 但不阻塞 PR merge（單列 issue tracking）。
   - **不跑 Windows runner**：Windows 開發者統一走 **WSL2 + Docker Desktop**（執行時等同 Linux），CI 加 Windows runner 成本高（GitHub Actions Windows runner 計價 2x、Docker-in-Docker 困難、librdkafka MSVC build 時常爆），ROI 低。
   - 開發者本機行為：macOS / WSL2 直接 `make dev`；Windows 原生（非 WSL2）僅支援 `make dev-host`（純 cargo run 啟動，不含 docker stack），明列於 §9 setup 文件。
6. ~~§5.2 seed 帳號使用之 `Tier 1 / Tier 2` 命名是否與 SPEC-005 KYC 分層字面一致？~~（已 Resolved 2026-05-05：SPEC-005 §2.1 釘住 canonical enum `TIER_0 / TIER_1 / TIER_2 / TIER_3`，本 SPEC §5.2 已同步。）
