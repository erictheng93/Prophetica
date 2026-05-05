# prophetica

> 趣味預測競猜娛樂遊戲平台 · Spec-Driven Development
> 域名（規劃中）：**prophetica.net**

---

## 產品概述 (Product Overview)

Prophetica 是一個 **趣味預測競猜娛樂遊戲平台**，讓玩家以站內**虛擬點數**對未公開事件結果（如選舉、體育賽事、經濟指標）進行預測。平台採用 **Order Book（訂單簿）** 機制（見 [ADR-0001](./docs/architecture/decisions/ADR-0001-order-book-with-dmm-over-lmsr.md)），由平台審查上架市場、執行結算，玩家依自身對事件機率的判斷以價格下單，與其他玩家**對手撮合**——平台僅作為**中立中介**，不持倉、不造市、不下場（見 [SPEC-001 §1.1](./specs/SPEC-001-legal-compliance.md) 三原則）。

入金端整合在地化通路（超商代碼 / 虛擬帳號 / 信用卡，見 [SPEC-016](./specs/SPEC-016-payment-gateway.md)）；用戶獲取方面採多層代理分潤體系（見 [SPEC-011](./specs/SPEC-011-affiliate-commission.md)）。

> 法律分類為**娛樂遊戲服務**——非博弈、非彩券、非衍生性金融商品。站內點數**不可兌回現金 / 加密貨幣 / 實體商品**（[SPEC-001 §1.1](./specs/SPEC-001-legal-compliance.md)）。

## 法律定位三大原則

1. **No Cash-out**：站內點數絕對不可換回現金、加密貨幣、實體商品。
2. **No House Position**：平台不持有任何合約倉位，不擔任造市商，不操作機器人下單。
3. **Full Audit Trail**：所有金流、撮合、結算、判定保留 7 年以上不可竄改稽核軌跡。

詳見 [SPEC-001 法律合規與司法管轄](./specs/SPEC-001-legal-compliance.md)。

> ⚠️ 上述三原則為**監管紅線**，等同創辦人刑事責任邊界。任何 PR / 設計提案若觸及（例：點數兌換、平台造市腳本、Geo-block 規避、跳過 KYC、移除責任博弈流程），一律 reject，不討論性能或商業權衡。

## 開發方法論

採用 **Spec-Driven Development**（SDD）：

- 所有 code 必須以對應 SPEC 章節為依據
- PR description 必須引用 `SPEC-XXX § N.N`，否則 reject
- SPEC 變更走獨立 PR，先審 SPEC 再寫 code；**不允許 SPEC 落後於程式碼**
- Production code 由 Codex 撰寫；Tech Lead 負責審視業務邏輯與 SPEC 符合度

## SPEC 索引

從 [`specs/SPEC-000-index.md`](./specs/SPEC-000-index.md) 開始閱讀。完整跨檔決策追蹤表亦位於 SPEC-000 §5。

### P0 — 第一行 production code 之前必須完成（6 份）

| ID | 標題 |
|----|------|
| [SPEC-001](./specs/SPEC-001-legal-compliance.md) | 法律合規與司法管轄框架 |
| [SPEC-002](./specs/SPEC-002-funds-safety.md) | 資金安全與虛擬點數帳本 |
| [SPEC-003](./specs/SPEC-003-matching-engine.md) | 撮合引擎詳規 |
| [SPEC-004](./specs/SPEC-004-settlement-dispute.md) | 結算與爭議處理 |
| [SPEC-015](./specs/SPEC-015-market-lifecycle.md) | 市場生命週期管理 |
| [SPEC-016](./specs/SPEC-016-payment-gateway.md) | 支付通路整合 |

### P1 — 公開 Beta 前完成（6 份）

| ID | 標題 |
|----|------|
| [SPEC-005](./specs/SPEC-005-kyc-aml.md) | KYC/AML 整合 |
| [SPEC-006](./specs/SPEC-006-responsible-gaming.md) | 責任博弈機制 |
| [SPEC-007](./specs/SPEC-007-observability.md) | 可觀測性與監控 |
| [SPEC-008](./specs/SPEC-008-disaster-recovery.md) | 災備與高可用 |
| [SPEC-009](./specs/SPEC-009-dmm-liquidity.md) | DMM 與流動性 |
| [SPEC-010](./specs/SPEC-010-security-antifraud.md) | 安全與反作弊 |

### P2 — Beta 後 90 天內完成（3 份）

| ID | 標題 |
|----|------|
| [SPEC-011](./specs/SPEC-011-affiliate-commission.md) | 代理分潤體系 |
| [SPEC-012](./specs/SPEC-012-customer-support.md) | 客服與爭議升級 |
| [SPEC-013](./specs/SPEC-013-cicd-release.md) | CI/CD 與發布 |

### 跨章節

| ID | 標題 |
|----|------|
| [SPEC-014](./specs/SPEC-014-tech-architecture.md) | 整體技術架構 |

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

詳見 [SPEC-014 整體技術架構](./specs/SPEC-014-tech-architecture.md) 與 [ADR-0004](./docs/architecture/decisions/ADR-0004-react-stack-due-to-kumo.md)。

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

詳見 [SPEC-014 §11](./specs/SPEC-014-tech-architecture.md)。

## 文件版本

當前 SPEC 套件版本：**v0.1.4**（2026-05-05）— 完成第二輪純技術交叉一致性 review；P0 擴為 6 份（加入市場生命週期 / 支付通路）。

完整變更紀錄見 [SPEC-000 §4](./specs/SPEC-000-index.md#4-變更紀錄)。

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
