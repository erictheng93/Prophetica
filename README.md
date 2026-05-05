# prophetica
域名將會註冊為：prophetica.net

> 趣味預測競猜娛樂遊戲平台 · Spec-Driven Development


## 專案性質

本專案是一個 **以虛擬點數運作的中性 P2P 預測撮合平台**，模仿 Kalshi 的 Order Book 模型，但以娛樂遊戲形式進行（**台灣司法管轄區內無任何出金通道**）。平台僅作為中性中介，不持倉、不造市，玩家之間自行對未公開事件下注，平台收取手續費。

## 法律定位三大原則

1. **No Cash-out**：站內點數絕對不可換回現金、加密貨幣、實體商品。
2. **No House Position**：平台不持有任何合約倉位，不擔任造市商，不操作機器人下單。
3. **Full Audit Trail**：所有金流、撮合、結算、判定保留 7 年以上不可竄改稽核軌跡。

詳見 [SPEC-001 法律合規與司法管轄](./specs/SPEC-001-legal-compliance.md)。

## 開發方法論

採用 **Spec-Driven Development**（SDD）：

- 所有 code 必須以對應 SPEC 章節為依據
- PR 必須在 description 引用 `SPEC-XXX § N.N`
- SPEC 變更走獨立 PR，先審 SPEC 再寫 code

## SPEC 索引

從 [`specs/SPEC-000-index.md`](./specs/SPEC-000-index.md) 開始閱讀。

| 階段 | 範圍 | SPEC |
|------|------|------|
| **P0 必須先完成** | 法律 / 帳本 / 撮合 / 結算 | SPEC-001 ~ 004 |
| **P1 公開 Beta 前** | KYC / RG / 監控 / 災備 / DMM / 安全 | SPEC-005 ~ 010 |
| **P2 Beta 後 90 天** | 推薦 / 客服 / CI/CD | SPEC-011 ~ 013 |
| **跨章節** | 整體技術架構 | SPEC-014 |

## 技術棧（簡）

- **後端**：Go 1.22 · Echo · gRPC · Kafka · PostgreSQL 16 · Redis 7
- **前端**：Next.js 15 (App Router, React 19) · TypeScript · `@cloudflare/kumo` · TailwindCSS · Zustand · Lightweight Charts
- **基礎設施**：Linode LKE (Tokyo Primary) + AWS (Singapore DR) · Cloudflare · ArgoCD
- **觀測**：Prometheus + Grafana + Loki + Tempo (OpenTelemetry)
- **KYC SaaS**：SumSub（主）+ Onfido（備）

詳見 [SPEC-014 整體技術架構](./specs/SPEC-014-tech-architecture.md)。

## 倉庫結構（規劃中）

```
prophetica/
├── apps/                  Next.js 15 前台、後台（React + Kumo）
├── services/              Go 後端服務
├── packages/              共用 protobuf / SDK
├── infra/                 Terraform / Helm / ArgoCD
├── specs/                 規格文件（本目錄）
├── docs/                  ADR / postmortem / diagrams
├── tools/                 壓測 / 遷移腳本
└── .github/workflows/     CI
```

實際 code base 由 Codex 依 SPEC 撰寫，本人（Tech Lead）審視業務邏輯與 SPEC 符合度。

## 啟動本地環境（待實作）

```bash
make dev
```

詳見 [SPEC-014 §11](./specs/SPEC-014-tech-architecture.md)。

## 文件版本

- `v0.1.0` — 2026-05-05 初始 SPEC 全套（14 份）

## 維護人

Tech Lead · 由 Operating Co. 簽派
