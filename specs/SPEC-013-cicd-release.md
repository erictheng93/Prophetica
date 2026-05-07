# SPEC-013 — CI/CD 與發布策略

> 優先級：**P2**
> 版本：v0.2.0 · 最後更新：2026-05-07
> 依賴：SPEC-007, SPEC-008, SPEC-010, SPEC-014, SPEC-017
> 被依賴：—

> **v0.2.0 變更**：(1) **H-3 CI gate 對齊**：§3.1 改為「SPEC-017 §8 之超集」明示繼承 bootstrap CI 全部 gate（pnpm/cargo/sqlx/buf/clippy/nextest/deny/geiger/sentinel-scan）+ 加上 prod-targeting gate（SAST / SCA / image scan / IaC / secret / migration / E2E）；§3.2 必過 gate 表合併重整 19 列，每列標註來源 SPEC；nightly 區塊新增 cargo-mutants / macos-latest / terraform plan diff（呼應 SPEC-017 §8.2 / §12 OQ #5）。(2) **H-6 Coverage 分層**：§3.3 新增分層覆蓋率表（ledger / 撮合 hot path ≥ 95% / 結算 ≥ 90% / 安全敏感 ≥ 85% / 其他預設 ≥ 80% / web ≥ 70% / mocks 不檢查），消除 SPEC-013（80%）vs SPEC-003 §16.1（100%）vs SPEC-003 §18（95%）三處數字不一致。(3) §3.1 prod approval cross-ref 至 SPEC-010 §9.3 MS-12。SPEC-013 v0.1.0 → v0.2.0；「依賴」加 SPEC-017。

---

## 1. Objective

確保程式碼從 commit 到 production 全程自動化、可重現、可回滾，並包含必要的安全與品質檢核點。

## 2. 環境分層

| 環境 | 用途 | 資料 | 部署方式 |
|------|------|------|---------|
| `local` | 開發者本機 | docker-compose | 手動 |
| `dev` | 整合測試 | 假資料 | 自動每次 main commit |
| `staging` | 預生產驗證 | 匿名化的 prod replica | 自動每次 release tag |
| `prod` | 生產 | 真實資料 | 手動 approval + 漸進式 |

## 3. CI Pipeline

### 3.1 GitHub Actions Workflow

> **本節 CI pipeline 為 SPEC-017 §8 bootstrap CI 之超集**：bootstrap 階段只跑 lint + test + sentinel-scan + 各 Rust gate；本節在此之上額外加上 prod-targeting gate（SAST / SCA / image scan / E2E / migration dry-run / 部署）。
>
> **繼承 SPEC-017 §8 之必過 gate**（Rust 工具鏈端）：
> - `pnpm install --frozen-lockfile`
> - `cargo build --workspace --locked`
> - `cargo sqlx prepare --check`（R-6）
> - `buf lint` / `buf breaking` / `verify generated stubs in sync`
> - `make lint`（cargo clippy `-D warnings` + cargo fmt --check + eslint + prettier）
> - `make test`（cargo nextest + pnpm turbo test）
> - `cargo deny check`
> - `cargo geiger` regression（R-5 unsafe 監測）
> - `sentinel-scan`（ADR-0001 §6 禁字 + tools/mocks 隔離 + R-1 紅線 grep gate `wallet_to_payable|cash_out_player_pts|redeem_points_to_cash`）

```
PR opened → run on every push to PR branch:
  1. [SPEC-017 §8 全部 gate]              ← 繼承
  2. Coverage gate（分層，見 §3.3）
  3. SAST (Semgrep)
  4. SCA (Trivy, Snyk)
  5. Container image scan (Trivy)
  6. IaC scan (Checkov)
  7. Secret scan (gitleaks pre-commit + CI)
  8. Build container image (no push)
  9. Database migration dry-run（CI ephemeral postgres）

PR merged to main → on merge:
  1. All of the above
  2. Push image to registry with tag {sha}（含 SBOM via syft）
  3. Image signing（cosign）
  4. Deploy to dev (auto)
  5. Run integration tests on dev
  6. Run E2E tests (Playwright)
  7. Notify Slack on failure

Release tag (v*) pushed:
  1. Promote dev image to staging
  2. Run staging smoke tests
  3. Wait for manual prod approval（呼應 SPEC-010 §9.3 MS-12 → 3-of-5 release-managers）
  4. Canary deploy to 1% of prod traffic
  5. Monitor for 30 min
  6. If green → progressive rollout 10% → 50% → 100%
  7. If red → auto rollback

Nightly（cron 02:00 UTC+8）:
  1. cargo-mutants on ledger + 撮合 hot path（SPEC-017 §8.2，R-4）
  2. macOS-latest test 跨平台驗證（SPEC-017 §12 OQ #5）
  3. terraform plan diff（配置 drift detection）
```

### 3.2 必須通過的 Gate（含繼承自 SPEC-017 §8）

| Gate | 來源 | 阻斷條件 |
|------|------|---------|
| `pnpm install --frozen-lockfile` | SPEC-017 §8 | pnpm-lock.yaml 不一致 |
| `cargo build --workspace --locked` | SPEC-017 §8 | Cargo.lock 不一致 / 編譯錯誤 |
| `cargo sqlx prepare --check` | SPEC-017 §8（R-6）| `.sqlx/` cache 與 schema drift |
| `buf lint / breaking / stubs in sync` | SPEC-017 §8 | proto 違規或 gen/ 未同步 regenerate |
| `make lint` | SPEC-017 §8 | clippy `-D warnings` / rustfmt / eslint / prettier 違反 |
| `make test` | SPEC-017 §8 | 任一測試 fail |
| **Coverage gate（分層，見 §3.3）** | **本節 H-6** | **平台預設 < 80%；ledger / 撮合 hot path < 95%** |
| `cargo deny check` | SPEC-017 §8 | 依賴授權 / advisory / banned crate 違反 |
| `cargo geiger` regression | SPEC-017 §8（R-5）| 新增 unsafe block 未經 ADR + 4-eye review |
| `sentinel-scan` ADR-0001 §6 | SPEC-017 §8 / SPEC-003 §12.1.3 | 禁字命中（auto_quote / market_balancer / house_book / probability_symmetry / auto_hedge）|
| `sentinel-scan` mocks isolation | SPEC-017 §8 | production code 引用 `tools/mocks` |
| **`sentinel-scan` R-1 點數出口** | **SPEC-011 §6.3** | **`wallet_to_payable` / `cash_out_player_pts` / `redeem_points_to_cash` 命中**|
| **SAST**（Semgrep）| **本節** | High / Critical |
| **SCA**（Trivy, Snyk）| **本節** | High / Critical CVE |
| **Container image scan**（Trivy）| **本節** | High / Critical |
| **IaC scan**（Checkov）| **本節** | High / Critical |
| **Secret scan**（gitleaks）| **本節** | 任何 hard-coded token / password |
| **Build** | **本節** | failure |
| **Migration dry-run** | **本節** | rollback 不可行 |

### 3.3 Coverage Gate 分層（H-6）

不同 crate 因業務風險不同，覆蓋率門檻分層執行：

| Crate / 模組 | 覆蓋率門檻 | 理由 | 來源 |
|-------------|-----------|------|------|
| `services/wallet-ledger-service/src/ledger/**` | **≥ 95%** | 帳本 INV-1 ~ INV-6 不變式核心；任何 mutation 直接影響資金安全 | SPEC-002 §11 / 本節 |
| `services/matching-engine/src/{book,match}/**` | **≥ 95%** | 撮合 hot path；確定性 / FIFO / STP 邏輯不可漏測 | SPEC-003 §16.1 / §18 |
| `crates/sdk/ledger/**` | **≥ 95%** | 共用 ledger client，所有 service 經此寫帳本 | SPEC-002 §12 OQ #1 |
| `services/settlement-worker/**` | **≥ 90%** | 結算批次原子性 + multi-sig 簽核流程 | SPEC-004 §7 |
| `services/kyc-service/**` | **≥ 85%** | KYC tier / 證件 hash / SaaS webhook idempotency | SPEC-005 §11 |
| `services/risk-antifraud-service/**` | **≥ 85%** | 多帳號偵測 / wash trading post-trade | SPEC-010 §4 |
| `services/payment-service/**` | **≥ 85%** | webhook HMAC + idempotency + chargeback 流程 | SPEC-016 §14 |
| **其他 services / crates 預設** | **≥ 80%** | 平台預設 baseline | 本節 |
| `apps/web/**` | **≥ 70%** | Next.js component coverage 比後端容忍度高（業務邏輯多在 server） | 本節 |
| `tools/mocks/**` | 不檢查 | 僅 dev / test 使用，不入 production binary | SPEC-017 §3.3 |

> 覆蓋率工具：Rust 用 [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov)（不依賴 nightly toolchain）；TypeScript 用 vitest 內建 coverage（`v8` provider）；報告 upload 至 Codecov 或自建 Grafana 看板。
>
> CI 跑 coverage 時間成本：增量 ~3-5 分鐘（cold build + 測試運行 instrumented binary），可放在獨立 job 與 lint/build 並行；不阻塞 PR merge 但若違反門檻必發 review-required label。

## 4. 部署策略（Progressive Delivery）

### 4.1 Canary

- 用 Argo Rollouts 或 Flagger 管理
- 流量切分基於 user_id hash（避免同一用戶在新舊版間跳）
- DMM 與重要 admin endpoint 走「late canary」（最後才放）

### 4.2 Feature Flag

- LaunchDarkly 或 Unleash
- 所有新功能默認 OFF，逐步開放
- Compliance / Engineering 可獨立 toggle
- 緊急停盤 flag (`engine.halted`) 為高敏感，需 2-of-N

### 4.3 Database Migration

- 採用 expand-and-contract pattern：
  1. 增加新 column (nullable)
  2. 部署寫入新 column 的 code
  3. backfill 歷史資料
  4. 部署只讀新 column 的 code
  5. drop 舊 column
- **嚴禁** rename / drop column 直接 deploy
- 大型 migration（影響 > 10M rows）必走 online migration（pgcompacttable / pg_repack）

### 4.4 Rollback 策略

| 觸發 | 動作 |
|------|------|
| Canary 期間任一 SLO 違反 | 自動 rollback |
| Error rate > 1% 持續 5 分鐘 | 自動 rollback |
| 工程主動 | `kubectl rollout undo` 或 Argo CLI |
| Database migration 失敗 | 復原至前一 schema（必須事前測試）|

## 5. Release Tag 規範

```
v{MAJOR}.{MINOR}.{PATCH}-{PRE_RELEASE}

例:
  v1.0.0           生產 release
  v1.0.0-rc.1      候選版
  v1.0.0-beta.1    Beta
```

- MAJOR：不向後兼容
- MINOR：向後兼容新功能
- PATCH：向後兼容修補

每個 release tag 對應：

- 自動產生 GitHub Release
- 自動推送 release notes（從 PR description 抽取）
- 觸發 staging 部署

## 6. Hotfix 流程

針對 production critical bug：

1. 從 prod tag 開 `hotfix/{issue}` 分支
2. 修補
3. PR + reduced gate（只跑 unit + 影響範圍 E2E）
4. Senior Engineer 簽核
5. 直接 deploy staging → smoke test
6. 立即 prod canary（5% 起步）
7. 完成後 cherry-pick 回 main

## 7. 部署敏感性矩陣

| 服務 | 部署窗 | 風險 | 特殊要求 |
|------|--------|------|---------|
| Matching Engine | 非交易高峰 | 極高 | 滾動升級 + 從 snapshot/WAL 恢復驗證 |
| Ledger Service | 非交易高峰 | 極高 | 4-eye review |
| Settlement | 非結算時段 | 極高 | Compliance 簽核 |
| API Gateway | 任意 | 中 | canary |
| Web Frontend | 任意 | 低 | canary |
| Admin Backend | 工作時段 | 中 | 內部 IP only |

## 8. 緊急停盤與部署互動

呼應 SPEC-008：

- 停盤期間禁止部署（除非為解除停盤所需）
- 每次部署後自動跑「stop-and-resume drill」（發 stop signal、確認、unstop、確認），不影響玩家但驗證機制
- 無 drill 通過 14 天 → 告警

## 9. 安全與合規檢查點

| 檢查 | 階段 | 工具 |
|------|------|------|
| 機密掃描（hard-coded token / password） | pre-commit + CI | gitleaks |
| 依賴漏洞 | CI | Trivy / Snyk |
| 容器映像 | CI | Trivy |
| IaC 漏洞 | CI | Checkov |
| 可觀測性配置 | CI | 自訂 lint，確認 metrics 命名 |
| 配置 drift | nightly | terraform plan diff |

## 10. 環境變數與配置管理

- 應用配置走環境變數 + Vault sidecar
- 配置變更走 GitOps（ArgoCD 監控 GitHub）
- 敏感配置走 Vault
- prod 配置與 staging 配置 diff 必須 < 10 行（避免「在 staging 跑得好，prod 不一樣」）

## 11. 部署紀錄與稽核

每次部署紀錄寫入：

- GitHub Release notes
- Slack `#deploys` 頻道
- Metric `proph_deployment_total{env, service, status}`
- 包含：Commit SHA、PR 號、部署人、時間、影響服務

## 12. Pre-launch Checklist（每個 prod release 必跑）

- [ ] 所有 PR 已 code review 並通過 4-eye（敏感模組）
- [ ] 所有 CI gate 通過
- [ ] Staging E2E 全綠
- [ ] Migration 已在 staging 跑過並可 rollback
- [ ] Feature flag 默認 OFF
- [ ] 監控 dashboard 已預備
- [ ] Runbook 已更新
- [ ] On-call 已被通知
- [ ] 用戶通告（如必要）

## 13. 驗收條件

- [ ] CI pipeline 完整跑通（PR / main / release tag）
- [ ] Canary 部署可動，流量可控
- [ ] Auto-rollback 機制驗證
- [ ] Migration expand-and-contract 模板驗證
- [ ] Feature flag 系統上線
- [ ] Release notes 自動生成
- [ ] 部署稽核日誌齊全

## 14. Open Questions

1. ArgoCD vs Flux？K8s GitOps 工具
2. Argo Rollouts vs Flagger？Canary 策略
3. Feature flag：LaunchDarkly（昂貴但可靠） vs Unleash（自建）？
4. Hotfix 流程是否需獨立分支策略？
5. SAST / SCA 偽陽性過濾流程（避免阻塞合理 merge）？
