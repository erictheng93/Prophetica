# SPEC-017 — Monorepo Bootstrap & Local Dev Environment

> 優先級：**P0（必須在第一行業務 code 之前完成）**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-014（架構定義）
> 被依賴：所有 SPEC（實作載體）

---

## 1. Objective

定義 prophetica 從「git clone 到 `make dev` 啟動本地全套服務」的最小可重現流程。本 SPEC 是「可實施規格」中**唯一不對應某個業務 service** 的 meta-SPEC，目的是讓 Codex / 新進工程師在第一個 PR 之前有明確的 bootstrap 步驟，避免每位實作者各自詮釋 SPEC-014 §5 的目錄結構。

### 1.1 In-scope

- Repository 骨架（Turborepo + pnpm workspaces + Go workspace）
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
├── services/
│   ├── api-gateway/            Go + Echo
│   ├── ws-gateway/             Go
│   ├── user-service/           Go
│   ├── kyc-service/            Go
│   ├── wallet-ledger-service/  Go
│   ├── market-service/         Go
│   ├── matching-engine/        Go (DaemonSet)
│   ├── position-service/       Go
│   ├── settlement-worker/      Go
│   ├── risk-antifraud-service/ Go
│   ├── referral-service/       Go
│   └── notification-service/   Go
├── packages/
│   ├── proto/                  buf workspace + .proto + generated stubs
│   │   └── v1/                 ← protobuf v1 集中地
│   ├── go-sdk/                 Go 內部共用程式庫（ledger / observability / errors）
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
│   └── load-test/                  k6 / Vegeta 壓測（暫空）
├── .github/
│   └── workflows/
│       ├── ci.yml                  PR / main 觸發
│       └── release.yml             tag 觸發
├── go.work
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
| Go | 1.22+ | `go.work` + `go.mod` go directive |
| Docker | 24+ | `make doctor` 檢查 |
| Docker Compose | v2（plugin 形式）| 同上 |
| buf | 1.30+ | `Makefile` install target |
| sqlc | 1.25+ | 同上 |
| golangci-lint | 1.55+ | 同上 |
| gofumpt | latest | 同上 |
| go-migrate | v4 | 同上 |
| make | 任意（Linux）/ Make for Windows / WSL2 | 文件提示 |

### 3.1 Windows 開發者特別說明

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

### 4.2 `go.work`

```go
go 1.22

use (
    ./services/api-gateway
    ./services/ws-gateway
    ./services/user-service
    ./services/kyc-service
    ./services/wallet-ledger-service
    ./services/market-service
    ./services/matching-engine
    ./services/position-service
    ./services/settlement-worker
    ./services/risk-antifraud-service
    ./services/referral-service
    ./services/notification-service
    ./packages/go-sdk
)
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
  - remote: buf.build/protocolbuffers/go
    out: packages/proto/gen/go
    opt: paths=source_relative
  - remote: buf.build/grpc/go
    out: packages/proto/gen/go
    opt: [paths=source_relative, require_unimplemented_servers=false]
  - remote: buf.build/bufbuild/es
    out: packages/proto/gen/ts
    opt: target=ts
```

### 4.5 `Makefile`（最低 target）

```makefile
.PHONY: doctor bootstrap dev down test lint proto migrate clean

doctor:
	@./tools/scripts/check-versions.sh

bootstrap:
	pnpm install
	go work sync
	$(MAKE) proto

dev:
	docker compose -f infra/docker-compose.local.yaml up -d
	pnpm turbo run dev --parallel

down:
	docker compose -f infra/docker-compose.local.yaml down -v

test:
	pnpm turbo run test
	go test ./services/... ./packages/go-sdk/...

lint:
	pnpm turbo run lint
	golangci-lint run ./services/... ./packages/go-sdk/...

proto:
	cd packages/proto && buf generate

migrate:
	./tools/scripts/migrate.sh up

clean:
	rm -rf node_modules apps/*/.next packages/proto/gen
	go clean -testcache
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
```

### 5.2 種子資料（Seed）

`make dev` 之後自動跑 `tools/scripts/seed.sh`：

- 建立 PG schema（執行 SPEC-002 / SPEC-015 migrations）
- 載入測試帳號 3 個：`alice`（Tier 2）、`bob`（Tier 1）、`dmm-001`（DMM 法人帳戶）
- 載入 1 個測試市場 `mkt_dev_demo`
- 不建立 `PLATFORM_SYSTEM_USER_IDS` 之 user 紀錄（呼應 SPEC-003 §12.1）

---

## 6. 各 service 最小 stub

每個 service 必須提供以下五要素，方能列為「bootstrap complete」：

| 要素 | 內容 |
|------|------|
| `cmd/main.go` | 啟動 + signal handling + graceful shutdown |
| `/healthz/live` | liveness（process up）|
| `/healthz/ready` | readiness（DB / Redis / Kafka 可達）|
| `/metrics` | Prometheus scrape |
| 結構化 JSON log | stdout，符合 SPEC-007 §4.1 格式 |

### 6.1 Go service 範例（`services/api-gateway/cmd/main.go`）

```go
package main

import (
    "context"
    "errors"
    "log/slog"
    "net/http"
    "os"
    "os/signal"
    "syscall"
    "time"

    "github.com/labstack/echo/v4"
)

func main() {
    log := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo}))
    slog.SetDefault(log)

    e := echo.New()
    e.HideBanner = true
    e.GET("/healthz/live", func(c echo.Context) error { return c.JSON(200, map[string]string{"status": "alive"}) })
    e.GET("/healthz/ready", func(c echo.Context) error { return c.JSON(200, map[string]string{"status": "ready"}) })
    // /metrics 由 promhttp 注入；省略

    addr := os.Getenv("LISTEN_ADDR")
    if addr == "" { addr = ":8080" }
    go func() {
        if err := e.Start(addr); err != nil && !errors.Is(err, http.ErrServerClosed) {
            log.Error("listen", "err", err)
            os.Exit(1)
        }
    }()
    log.Info("api-gateway started", "addr", addr)

    quit := make(chan os.Signal, 1)
    signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
    <-quit
    ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
    defer cancel()
    _ = e.Shutdown(ctx)
}
```

### 6.2 Next.js app 範例（`apps/web/`）

- `app/page.tsx`：顯示 "prophetica web — bootstrapping"
- `app/api/health/route.ts`：回 `{status: "ok"}`
- `next.config.mjs`：含 `@cloudflare/next-on-pages` 設定（呼應 ADR-0004）

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

- 開發環境：`.env` 透過 `direnv` 或 docker-compose 注入
- 生產環境：Vault sidecar agent（呼應 SPEC-010 §3.3）
- 禁區：**禁止**將 secret 寫入 `ConfigMap` 或 image
- 禁區：**禁止**在 Go code 中 hardcode default secret（`os.Getenv` 沒值就 fail-fast）

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
      - uses: actions/setup-go@v5
        with:
          go-version-file: 'go.work'
      - uses: bufbuild/buf-setup-action@v1

      - name: pnpm install
        run: pnpm install --frozen-lockfile
      - name: go work sync
        run: go work sync
      - name: buf generate
        run: cd packages/proto && buf generate
      - name: buf lint
        run: cd packages/proto && buf lint
      - name: buf breaking (vs main)
        run: cd packages/proto && buf breaking --against '.git#branch=main'

      - name: lint
        run: make lint
      - name: test
        run: make test

  sentinel-scan:
    runs-on: ubuntu-22.04
    needs: lint-and-test
    steps:
      - uses: actions/checkout@v4
      - name: ADR-0001 §6 forbidden words
        run: |
          if grep -rn -E 'auto_quote|market_balancer|house_book|probability_symmetry|auto_hedge' \
               --include='*.go' services/ packages/ ; then
            echo "::error::ADR-0001 §6 forbidden words detected"; exit 1; fi
```

### 8.1 必過 gate

| Gate | 阻斷條件 |
|------|---------|
| `pnpm install` | lock 檔不一致 |
| `go work sync` | go.work 與各 module go.mod 不一致 |
| `buf lint` | proto 違反 lint |
| `buf breaking` | 對 main 有 breaking change（未經 review）|
| `make lint` | golangci-lint / eslint / prettier 違反 |
| `make test` | 任一測試 fail |
| `sentinel-scan` | ADR-0001 §6 禁字命中 |

---

## 9. 開發者首次設定流程

新進工程師 / Codex 第一次 clone 後執行：

```bash
git clone https://github.com/erictheng93/prophetica.git
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
| SPEC-002 §4.3 | migration 工具（go-migrate）的腳本位置 |
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
- [ ] `buf generate` 為每個 .proto 產出 Go + TS stub 並提交至 `packages/proto/gen/`

---

## 12. Open Questions

1. `packages/proto/gen/` 是否 commit 進 git？選項：(a) commit 確保跨機器一致；(b) `.gitignore` 並由 CI 重產。建議 (a)，避免本地未跑 `buf generate` 導致 import error。
2. `.env` 配置注入採 `direnv` vs `docker-compose env_file` vs `dotenv-cli`？三者皆可，需選一以避免分歧。
3. mock servers（`tools/mocks/sumsub` / `payment`）是否獨立 repo 或同 monorepo？建議同 monorepo（避免版本同步成本）。
4. Windows 主機之 host networking 限制如何處理 hostNetwork 撮合引擎部署？本地 dev 不啟用 hostNetwork（僅 prod K8s 啟用），由 SPEC-008 §3 監督。
5. CI matrix（macOS / Linux / Windows）是否啟用？建議 Linux only（生產環境一致），開發者 Windows 走 WSL2。
