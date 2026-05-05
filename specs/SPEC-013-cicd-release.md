# SPEC-013 — CI/CD 與發布策略

> 優先級：**P2**
> 版本：v0.1.0 · 最後更新：2026-05-05
> 依賴：SPEC-007, SPEC-008, SPEC-010, SPEC-014
> 被依賴：—

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

```
PR opened → run on every push to PR branch:
  1. Lint (cargo clippy `-D warnings`, eslint)
  2. Format check (cargo fmt --check, prettier)
  3. Unit tests (with coverage > 80%)
  4. SAST (Semgrep)
  5. SCA (Trivy, Snyk)
  6. Build container image (no push)
  7. Database migration dry-run

PR merged to main → on merge:
  1. All of the above
  2. Push image to registry with tag {sha}
  3. Deploy to dev (auto)
  4. Run integration tests on dev
  5. Run E2E tests (Playwright)
  6. Notify Slack on failure

Release tag (v*) pushed:
  1. Promote dev image to staging
  2. Run staging smoke tests
  3. Wait for manual prod approval (3-of-5 from release-managers team)
  4. Canary deploy to 1% of prod traffic
  5. Monitor for 30 min
  6. If green → progressive rollout 10% → 50% → 100%
  7. If red → auto rollback
```

### 3.2 必須通過的 Gate

| Gate | 阻斷條件 |
|------|---------|
| Lint | 任何 warning |
| Format | 任何不符規範 |
| Test | 任何 fail / coverage < 80% |
| SAST | High / Critical |
| SCA | High / Critical CVE |
| Build | failure |
| Migration | rollback 不可行 |

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
