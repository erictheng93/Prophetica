# prophetica

> 趣味預測競猜娛樂遊戲平台 · Spec-Driven Development
> 域名（規劃中）：**prophetica.net**

---

## 產品概述 (Product Overview)

Prophetica 是一個 **趣味預測競猜娛樂遊戲平台**，讓玩家以站內**虛擬點數**對未公開事件結果（如選舉、體育賽事、經濟指標）進行預測。平台採用 **Order Book（訂單簿）** 機制（見 [ADR-0001](./docs/architecture/decisions/ADR-0001-order-book-with-dmm-over-lmsr.md)），由平台審查上架市場、執行結算，玩家依自身對事件機率的判斷以價格下單，與其他玩家**對手撮合**。

入金端整合在地化通路（超商代碼 / 虛擬帳號 / 信用卡，見 [SPEC-016](./specs/SPEC-016-payment-gateway.html)）；用戶獲取方面採多層代理分潤體系（見 [SPEC-011](./specs/SPEC-011-affiliate-commission.html)）。

> 站內點數用於顯示餘額、鎖倉與結算結果；實際使用規則由業主與營運方另行維護。

## 技術邊界

1. 站內點數以 ledger account、wallet balance、locked balance 與 settlement entry 表示。
2. 撮合引擎以訂單簿處理玩家訂單，系統帳號保留在 sentinel set 中避免誤進一般下單流程。
3. 金流、撮合、結算、判定保留事件紀錄，供客服、工程與授權稽核流程追查。

法律分類與營運司法管轄事項由業主與營運方另行維護；本 repo 保留技術可實作規格。

> 法律分類、服務地區與營運條款由業主與營運方另行維護；本 repo 僅保留技術可實作規格與工程約束。

## 開發方法論

採用 **Spec-Driven Development**（SDD）：

- 所有 code 必須以對應 SPEC 章節為依據
- PR description 必須引用 `SPEC-XXX § N.N`，否則 reject
- SPEC 變更走獨立 PR，先審 SPEC 再寫 code；**不允許 SPEC 落後於程式碼**
- Production code 由 Codex 撰寫；Tech Lead 負責審視業務邏輯與 SPEC 符合度

## SPEC 索引

從 [`specs/SPEC-000-index.html`](./specs/SPEC-000-index.html) 開始閱讀。完整跨檔決策追蹤表亦位於 SPEC-000 §5。

### P0 — 第一行 production code 之前必須完成（6 份）

| ID | 標題 |
|----|------|
| [SPEC-002](./specs/SPEC-002-funds-safety.html) | 資金安全與虛擬點數帳本 |
| [SPEC-003](./specs/SPEC-003-matching-engine.html) | 撮合引擎詳規 |
| [SPEC-004](./specs/SPEC-004-settlement-dispute.html) | 結算與爭議處理 |
| [SPEC-015](./specs/SPEC-015-market-lifecycle.html) | 市場生命週期管理 |
| [SPEC-016](./specs/SPEC-016-payment-gateway.html) | 支付通路整合 |
| [SPEC-017](./specs/SPEC-017-monorepo-bootstrap.html) | Monorepo bootstrap 與本地開發 |

### P1 — 公開 Beta 前完成（10 份）

| ID | 標題 |
|----|------|
| [SPEC-005](./specs/SPEC-005-kyc-aml.html) | KYC/AML 整合（✅ 可視為完成） |
| [SPEC-006](./specs/SPEC-006-responsible-gaming.html) | 責任博弈機制 |
| [SPEC-007](./specs/SPEC-007-observability.html) | 可觀測性與監控 |
| [SPEC-008](./specs/SPEC-008-disaster-recovery.html) | 災備與高可用 |
| [SPEC-009](./specs/SPEC-009-dmm-liquidity.html) | DMM 與流動性 |
| [SPEC-010](./specs/SPEC-010-security-antifraud.html) | 安全與反作弊 |
| [SPEC-012](./specs/SPEC-012-customer-support.html) | 客服與爭議升級 |
| [SPEC-013](./specs/SPEC-013-cicd-release.html) | CI/CD 與發布 |
| [SPEC-018](./specs/SPEC-018-auditability-evidence-pack.html) | 稽核與合規證據化 |
| [SPEC-019](./specs/SPEC-019-incident-response-sre.html) | 事件與事故回應 |

### P2 — Beta 後 90 天內完成（2 份）

| ID | 標題 |
|----|------|
| [SPEC-011](./specs/SPEC-011-affiliate-commission.html) | 代理分潤體系 |
| [SPEC-020](./specs/SPEC-020-analytics-reporting.html) | 分析與報表 |

### 跨章節

| ID | 標題 |
|----|------|
| [SPEC-014](./specs/SPEC-014-tech-architecture.html) | 整體技術架構 |

## 架構決策（ADR）

決策紀錄位於 [`docs/architecture/decisions/`](./docs/architecture/decisions/)。

| ID | 標題 |
|----|------|
| [ADR-0001](./docs/architecture/decisions/ADR-0001-order-book-with-dmm-over-lmsr.md) | 撮合模型採 Order Book + DMM，捨棄 LMSR |
| [ADR-0002](./docs/architecture/decisions/ADR-0002-three-company-corporate-structure.md) | 三公司結構（Holding / Operating / Tech）|
| [ADR-0003](./docs/architecture/decisions/ADR-0003-double-entry-ledger.md) | 採用雙簿記帳本 |
| [ADR-0004](./docs/architecture/decisions/ADR-0004-react-stack-due-to-kumo.md) | 前端棧由 Nuxt 3 切換為 Next.js 15 / React 19（Cloudflare Kumo）|

## 技術棧（簡）

- **後端**：Rust 1.78+ · axum · tonic (gRPC) · sqlx · tokio · Kafka · PostgreSQL 16 · Redis 7
- **前端**：Next.js 15 (App Router, React 19) · TypeScript · `@cloudflare/kumo` · TailwindCSS · Zustand · Lightweight Charts
- **基礎設施**：Linode LKE (Tokyo Primary) + AWS (Singapore DR) · Cloudflare · ArgoCD
- **觀測**：Prometheus + Grafana + Loki + Tempo (OpenTelemetry)
- **KYC SaaS**：SumSub（主）+ Onfido（備）

詳見 [SPEC-014 整體技術架構](./specs/SPEC-014-tech-architecture.html) 與 [ADR-0004](./docs/architecture/decisions/ADR-0004-react-stack-due-to-kumo.md)。

## 倉庫結構

```
prophetica/
├── apps/                  Next.js 15 前台、後台（React + Kumo）       （規劃中）
├── services/              Rust 後端 service crates                     （規劃中）
├── crates/                Rust 共用 lib（sdk / proto）                 （規劃中）
├── packages/              共用 protobuf 來源 / TS SDK                  （規劃中）
├── infra/                 Terraform / Helm / ArgoCD                    （規劃中）
├── specs/                 規格文件（SPEC-000 ~ 016）                   ✅
├── docs/architecture/     ADR / 圖表                                   ✅
├── design/                早期視覺探索（apple / airbnb / lamborghini） ✅
├── tools/                 壓測 / 遷移腳本                              （規劃中）
└── .github/workflows/     CI                                           （規劃中）
```

實際 code base 由 Codex 依 SPEC 撰寫。`design/` 內為三份風格探索 landing page，**非 production**，僅供視覺方向討論。

## 啟動本地環境（待實作）

```bash
make dev
```

詳見 [SPEC-014 §11](./specs/SPEC-014-tech-architecture.html)。

## 文件版本

當前 SPEC 套件版本：**v0.3.3**（2026-05-09）— 新增 Public Beta Launch Gate，將客服、發布安全、稽核證據化上移為 P1 launch gate，並修復 HTML 規格產物漂移。

完整變更紀錄見 [SPEC-000 §4](./specs/SPEC-000-index.html#4-變更紀錄)。

## 維護人

Tech Lead · 由 Operating Co. 簽派
# SPEC-017 Bootstrap

This repository now uses the SPEC-017 monorepo scaffold:

- Rust 1.85 / edition 2024 Cargo workspace for service stubs and shared crates.
- pnpm 9 + Turborepo for `apps/*` and `packages/*`.
- `buf.yaml` and `buf.gen.yaml` consume `packages/proto/v1/*.proto` and emit committed Rust and TypeScript stubs.
- `infra/docker-compose.local.yaml` provides the local bootstrap stack.

Common commands:

```bash
make doctor
make bootstrap
make dev
make test
make lint
```

The first PR intentionally contains only bootstrap stubs. Business logic remains out of scope for SPEC-017.
