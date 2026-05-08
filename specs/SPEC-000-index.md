# SPEC-000 — 規格總索引（Master Index）

> **prophetica** — 趣味預測娛樂遊戲平台
> 文件版本：v0.3.2 · 最後更新：2026-05-07 · 維護人：Tech Lead

---

## 0. 文件性質

本目錄下所有 `SPEC-*.md` 文件均為**可實施規格**（Implementable Specification），遵循 SDD（Spec-Driven Development）方法論。

- 每份 SPEC 都是 Codex 開發任務的輸入。
- 所有實作 PR 必須在 description 中引用 `SPEC-XXX § N.N` 章節。
- 任何 SPEC 變更須先提 PR 審閱，再進行對應實作。
- SPEC 與 code 同步進入 git，不允許 SPEC 落後於程式碼。

## 1. 專案定位

| 項目 | 內容 |
|------|------|
| 中文名稱 | 趣味預測競猜娛樂遊戲平台 |
| 英文代號 | prophetica |
| 法律分類 | **娛樂遊戲服務**（非博弈、非彩券、非衍生性金融商品） |
| 玩家行為 | 以站內**虛擬點數**對未公開事件結果進行預測，與其他玩家撮合 |
| 平台角色 | **中立撮合平台**，僅收手續費，不持有合約倉位、不做造市商 |
| 出金政策 | **台灣司法管轄區內無任何出金通道**；點數僅可在站內使用，不可兌換現金 / 虛擬貨幣 / 實體商品 |
| 主要市場 | 亞太娛樂用戶 |
| 嚴格 Geo-block | 以 [SPEC-001 §2](./SPEC-001-legal-compliance.md#2-geo-block-矩陣) 的 🔴 紅名單機制與多層防禦為準；首發保守名單待法律意見書最終確認 |

## 2. SPEC 清單

### P0 — 必須在第一行 code 之前完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-001](./SPEC-001-legal-compliance.md) | 法律合規與司法管轄框架 | 合規三原則、Geo-block、VPN 政策、ToS、監管稽核資料、隱私與 AML | 🟢 v0.1.1 |
| [SPEC-002](./SPEC-002-funds-safety.md) | 資金安全與虛擬點數帳本 | 雙簿記、隔離帳戶、對帳機制、Frozen 子科目、**INV-6 點數出口白名單（R-1，ADR-0006 落地，含 chargeback carve-out v0.1.8）**、§9.3 最終一致性窗口（T-5）、Phase 1 monthly partition（T-3） | 🟢 v0.1.8 |
| [SPEC-003](./SPEC-003-matching-engine.md) | 撮合引擎詳規 | Order Book、訂單類型、撮合演算法、API、SLO、**uid=4 sentinel 移除（R-2）**、§17.2 etcd-driven 市場路由（T-4）、§17.3 熱冷市場 thread model 分流（T-6）| 🟢 v0.2.3（Rust） |
| [SPEC-004](./SPEC-004-settlement-dispute.md) | 結算與爭議處理 | Oracle、結算狀態機、申訴窗口、batch 500 + off-peak（T-12）| 🟢 v0.1.2 |
| [SPEC-015](./SPEC-015-market-lifecycle.md) | 市場生命週期管理 | 市場創建、上架審查、內容紅線、開盤排程、強制下架 | 🟢 v0.1.1 |
| [SPEC-016](./SPEC-016-payment-gateway.md) | 支付通路整合 | 通路選型、Webhook、Chargeback ≥ 1 統一封號（H-5）、跨幣別、PCI 範圍 | 🟢 v0.1.1 |
| [SPEC-017](./SPEC-017-monorepo-bootstrap.md) | Monorepo Bootstrap & Local Dev | Repo 骨架、Docker Compose、Makefile、CI bootstrap、PR #1 acceptance checklist、Mutation 門檻分階段（T-7）、CI runner 4-cores（T-8）、§11.3 PoC gating（T-9 / T-10） | 🟢 v0.2.6（Rust） |

### P1 — 必須在公開 Beta 之前完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-005](./SPEC-005-kyc-aml.md) | KYC/AML 整合 | 分層 KYC、SaaS 整合、制裁名單篩查 | ⚠️ Backend in-memory contract / P0 follow-up · v0.1.3 |
| [SPEC-005a](./SPEC-005a-sumsub-frontend-contract.md) | SumSub 前端整合契約 | `KycIdentityProvider` 抽象、Tier 2 page state machine、idempotency / polling / CSP 紀律 | ⚠️ Contract draft · v0.1.0 |
| [SPEC-006](./SPEC-006-responsible-gaming.md) | 責任博弈機制 | 自我排除、限額、冷靜期 | 🟢 v0.1 |
| [SPEC-007](./SPEC-007-observability.md) | 可觀測性與監控 | Metrics/Log/Tracing、SLO、告警、**§6.4 跨服務 Latency Budget**（下單熱路徑 11 hop × P99 時間預算）| 🟢 v0.2.1 draft |
| [SPEC-008](./SPEC-008-disaster-recovery.md) | 災備與高可用 | **RTO/RPO 拆 region-local + cross-region 兩層（T-2）**、跨區備援、Failover 演練、§7 客戶端容錯可執行契約（v0.2.1 F-2）、§9.3 cross-ref SPEC-010 §9.3 MS-9（v0.2.1 F-1）| 🟢 v0.2.1 |
| [SPEC-009](./SPEC-009-dmm-liquidity.md) | DMM 與流動性 | 做市商計畫、API、rebate；**§5 冷啟動補貼整段移除（R-2）** | 🟢 v0.2.1（Rust） |
| [SPEC-010](./SPEC-010-security-antifraud.md) | 安全與反作弊 | 裝置指紋、行為分析、洗售偵測、**§9.3 Multi-sig 治理中央表**、EdDSA mobile PoC（T-9，PoC plan 落地）、sqlx 編譯時 SQL（H-4）、Chargeback ≥ 1（H-5）| 🟢 v0.2.2 |

### P2 — 公開 Beta 後 90 天內完成

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-011](./SPEC-011-affiliate-commission.md) | 代理分潤體系 | 單層介紹費 + 交易佣金、業務 B2B 路徑（**獨立法律關係，不從玩家錢包出口；R-1**）、稅務扣繳、§4.1.2 延遲 30 天發放 + §4.6 月度 / 終身 / 群組上限（G-3 / G-8）| 🟢 v0.2.1 |
| [SPEC-012](./SPEC-012-customer-support.md) | 客服與爭議升級 | SLA、工單系統 | 🟢 v0.1 |
| [SPEC-013](./SPEC-013-cicd-release.md) | CI/CD 與發布 | Pipeline（SPEC-017 §8 超集，H-3）、Coverage 分層（H-6）、Feature Flag、回滾 | 🟢 v0.2.0 |

### 跨章節文件

| ID | 標題 | 摘要 | 狀態 |
|----|------|------|------|
| [SPEC-014](./SPEC-014-tech-architecture.md) | 整體技術架構 | 服務拓撲、技術棧（Rust 1.85 / edition 2024，**ADR-0005 governance**）、Repo 結構、§3.2.1 Cloudflare Pages Edge runtime PoC（T-10，PoC plan 落地）、§4.1 MVP 合併部署 advisory（T-1）、§4.1.1 當前 12-binary 拓撲與轉 7-binary 觸發條件 | 🟢 v0.2.6 |

## 3. 全域名詞表（Glossary）

| 名詞 | 中文 | 定義 |
|------|------|------|
| Market | 市場 / 合約 | 一個可預測的未公開事件，含 YES/NO 兩個 Outcome |
| Outcome | 結果腿 | 市場的兩種結果之一（YES 或 NO） |
| Order | 訂單 | 玩家對某個 Outcome 在特定價格下單的意圖 |
| Trade | 成交 | 兩張對手方訂單撮合形成的交易紀錄 |
| Position | 持倉 | 玩家在某個 Outcome 上的累計持有股數 |
| Settlement | 結算 | 市場到期後依官方判定結果，將持倉轉為點數 |
| Tick | 最小價格單位 | 1 cent（0.01 點） |
| Lot | 最小數量單位 | 1 股 = 1 點面額 |
| Points | 站內點數 | 玩家儲值購買的虛擬遊戲點數，**不可兌回現金** |
| DMM | Designated Market Maker | 受邀的外部做市商 |
| KYC | Know Your Customer | 客戶身份驗證 |
| AML | Anti-Money Laundering | 反洗錢機制 |

## 4. 變更紀錄

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-07 | v0.3.2 | Tech Lead | **SPEC-011 v0.2.1 pre-implementation gap closure（G-1 ~ G-8）**：Tech Lead 對 SPEC-011 v0.2.0 進行落地前審視，發現合規層（§2 / §6）成熟但落地細節有 8 個技術 gap 將導致 Codex 動工後 PR 退回。本輪閉環如下：(1) **G-1** §3.2 推薦碼演算法釘死：BLAKE3 6-byte hex（12 chars）+ Crockford Base32 4-char random + collision retry 5 次；(2) **G-2** §3.3.1 親屬偵測升級為 SPEC-010 §4.2.1 `user_link_score` 子規則 `affiliate_kinship_link`，含 6 條偵測特徵 / 權重 + 雙重 gate（註冊前 / 發放前）+ Compliance MS-4 4-eye 裁決；(3) **G-3** §4.1 達標獎勵改為**延遲 30 天發放**模型，徹底繞開 INV-6 clawback 衝突（從未發放 → 無物可追回）；(4) **G-4** §4.2 交易佣金獨立每日批次 txn + `commission_accruals` staging 表，trade transaction entry 維持 SPEC-002 §4.4.3 兩條不變；(5) **G-5** §4.4 chargeback 反沖細化：介紹獎勵走 G-3 延遲路徑無需 clawback；持續佣金封號自動停止；封號前已支付佣金**不追回**（保善意第三方 + 不破 INV-6 + cost < CAC 可吸收）；(6) **G-6** §4.5 ledger entry 補 sign convention cross-ref SPEC-002 §4.3 direction =±1 規則；(7) **G-7** §5.1 「推薦樹」用詞改為「推薦清單（flat list）」避免暗示 MLM；(8) **G-8** §4.6 推薦獎勵上限：月度 20 個介紹 / 終身 200 個 / 月度佣金 TWD 50k / 同 IP-fingerprint 群組合併計算，超出進 EDD 4-eye review。§9 驗收條件由原 7 項擴充為 9.1 後端（9 項，Codex owner）+ 9.2 前端（3 項，Tech Lead owner）+ 9.3 跨 SPEC 依賴（2 項）+ 9.4 法律 / 業務（2 項）。§10 OQ #1 標 Resolved，新增 OQ #6（上限調整 governance）。本檔 §5 新增 Q-8 跨檔決策追蹤條目（v0.2.1 8 個 gap 之 single source of truth）。SPEC-011 「依賴」由 4 份擴增為 5 份（加 SPEC-016）。SPEC-011 v0.2.0 → v0.2.1。 |
| 2026-05-07 | v0.3.1 | Tech Lead | **SPEC-008 v0.2.1 patch（F-1 ~ F-5 縫補）**：(1) **F-1 governance drift fix**：§9.3 Kill switch 解除明示 cross-ref SPEC-010 §9.3 **MS-9**，禁止 admin UI 為 MS-9 自刻流程（單一 multi-sig component 原則）；(2) **F-2 客戶端容錯契約化**：§7.1 / §7.2 / §7.3 / §7.4 由描述性敘述升級為可執行技術契約 — WS reconnect（500ms / 倍率 2.0 / jitter ±20% / cap 30s / max 30 attempts / heartbeat 15s）+ Idempotency-Key（UUID v7）+ Reconcile（per-subscription monotonic seq + 60s tail）+ Fail-soft 模式（polling 降級 + disable 下單 + 手動重試）+ retry budget 反 pattern 警示（呼應 SPEC-007 §6.4.4）；(3) **F-3 stale OQ 清理**：§13 OQ #5「監管最低備份保留期」標 Resolved（已被 SPEC-001 §1.1 ③ 7 年 + Q-2 覆蓋）；(4) **F-4 Redis 邊界釐清**：§5.2 補 inline 註明「Lock / Idempotency 落 PG，**不**存 Redis」呼應 SPEC-002 §6.1；(5) **F-5 future-proof**：§3.1 區域佈局圖補 Cloudflare Edge runtime（PoC-B）fallback semantics 待回填；(6) **前端落地**：`apps/web/lib/idempotency.ts`（UUID v7 + withIdempotencyKey）+ `apps/web/lib/ws-client.ts`（reconnecting WebSocket + reconcile）+ `apps/web/components/connection-status-banner.tsx`（單一 banner 對應 §7.3 fail-soft 與 §9.2 HALTED 兩態）；對應 unit tests 跑 `node:test` 通過 + `tsc --noEmit` 過。SPEC-008 v0.2.0 → v0.2.1。 |
| 2026-05-07 | v0.3.0 | Tech Lead | **PoC artifacts 落地 round 8**（最後 pre-flight 工程準備）：(1) **PoC-A（EdDSA / LIFF）**：建立 [`docs/poc/eddsa/`](../docs/poc/eddsa/) — `README.md`（playbook + desk research + acceptance + dual-algo 退路）/ `check-support.html`（Web Crypto Ed25519 三項 standalone 檢測）/ `server-token-sample.rs`（Rust + jsonwebtoken 0.9 EdDSA token issue + verify reference，含 cookie HTTP-only / SameSite=Lax 配置）。Desk research 結論：iOS Safari 17+ / Android Chrome / Cloudflare Workers / 各 mobile JWT lib 皆支援 Ed25519；LIFF webview engine ≈ host Safari/Chrome；舊裝置（iOS 15- / Android 9-）為已知風險點需實機驗證。(2) **PoC-B（Cloudflare Edge runtime）**：建立 [`docs/poc/edge-runtime/`](../docs/poc/edge-runtime/) — `README.md`（5 場景 playbook + desk research + acceptance + 退路矩陣）/ `scenarios.md`（5 場景之 minimal Next.js 15 code snippets：KYC iframe + WebSocket fan-out + Server Actions + fetch behaviour + env / secret binding）。Desk research 結論：A/B/D/E preliminary pass；C 部分風險（某些 SaaS SDK 之 Node.js API 依賴會在 Edge runtime build fail）。(3) **PRE-FLIGHT-POCS.md v1.0 → v1.1**：status 從 🟡 Pending 升級為 🟢 Desk Research Done · 🟡 Live Verification Pending；§3.5 / §4.4 結構化回填欄位（Step / Scenario × ☐ Pass/Fail checkbox + 失敗時 offending 標記）。(4) PoC 執行成本評估：PoC-A ~ 半天 + 1 LIFF channel + 2 phones；PoC-B ~ 1-2 天 + 1 Cloudflare Pages preview + ngrok。(5) **PR #1 開發路徑解鎖**：用戶可在 PR #1 開發期間並行跑 live PoC；artifacts 已就緒消除「PoC 阻塞」之 critical path。 |
| 2026-05-07 | v0.2.9 | Tech Lead | **程式碼 ↔ SPEC 同步收斂 round 7**（agent 驗證 → R-1/R-2 補齊 + 殘餘 gap 對齊）：(1) **R-1/R-2 程式碼補齊**（commit `430643c`）：sentinel 重構為 sorted const slice + `RESERVED_REMOVED_USER_IDS`、INV-6 application validate + DB CONSTRAINT TRIGGER 雙層、CI sentinel-scan job 補 R-1/R-2 grep gate、CI runner 切 `ubuntu-latest-4-cores`、KUMO_NPM_TOKEN 殘留移除、cargo-mutants 門檻調 60；87 unit tests pass / clippy clean / fmt clean；INV-6 chargeback carve-out 補正（SPEC-002 v0.1.8、ADR-0006 v1.0.1）。(2) **SPEC-005 §0 標記降級**：v0.1.2「✅ Complete」→ v0.1.3「⚠️ Backend Complete / Frontend Pending SumSub Integration」；新增 §0.1 backend 已完成清單、§0.2 待完成清單（SumSub iframe / Tier 2/3 OCR / self-exclusion / batch scheduler / income stale / EdDSA PoC）、§0.3 PR #1 acceptance 條件。SDD 紀律：spec 反映現況，不誤導 Compliance。(3) **`.env.example` sweep**：移除殘留 `KUMO_NPM_TOKEN=`（v0.2.3 erratum 後不需要）、補 observability port（PROMETHEUS / GRAFANA / TEMPO_OTLP_GRPC / LOKI / MAILHOG SMTP+UI）、補 frontend port（WEB / ADMIN / LIFF）、補 `NEXT_PUBLIC_LIFF_ID`、補檔頭註記 SPEC-017 §7.1 / SPEC-010 §3.3 / Vault sidecar 警示。(4) **SPEC-014 §4.1.1 對齊現況**：紀錄 round 7 程式碼驗證結果「當前採 12-binary 拓撲」+ 採 12-binary 之 acceptance 補強（PR description 須宣告 latency budget 壓測承諾）+ §4.1.1.1 轉 7-binary 之 4 條觸發條件 + 簡述轉換流程；SPEC-014 v0.2.5 → v0.2.6。(5) Agent 驗證澄清：admin / liff stub 對 SPEC-017 §6.3 acceptance（health route + middleware + LIFF init + fail-fast）已落地；apps/web/app/api/health/route.ts 已存在（agent 之前誤報）。版本 bump：SPEC-002 v0.1.7 → v0.1.8、SPEC-005 v0.1.2 → v0.1.3、SPEC-014 v0.2.5 → v0.2.6；ADR-0006 v1.0 → v1.0.1。 |
| 2026-05-07 | v0.2.8 | Tech Lead | **動工前最後兩件 round 6**（ADR-0005 retroactive + PoC plan 文件化）：(1) **ADR-0005 新增（Accepted, retroactive）**：補做 SPEC-014 v0.2.0「後端 Go → Rust」切換之 governance ADR。涵蓋 driver（GC pause / 不變式型別化 / 業界對標 Kalshi）、4 個 considered options（維持 Go / Rust+Go 混搭 / 全 Rust / C++ hot path）、為何選全 Rust 之具體論證（單語言 SDK 維運成本 < CRUD borrow checker 成本）、Negative consequences 與 Mitigation Plan、Notes for Codex 6 條紀律。SPEC-014 §3.1 v0.2.0 變更說明補 cross-ref 至 ADR-0005；SPEC-014 v0.2.4 → v0.2.5。(2) **PoC plan 文件落地**：建立 [`docs/poc/PRE-FLIGHT-POCS.md`](../docs/poc/PRE-FLIGHT-POCS.md)，涵蓋 PoC-A（EdDSA / LIFF SDK 三步驟驗證 + 退路）+ PoC-B（Cloudflare Edge runtime 5 場景驗證 + 退路）+ §5 PoC 完成後 SPEC 回填規則 + §6 失敗 escalation 流程；SPEC-010 §3.1.2 / SPEC-014 §3.2.1 補 cross-ref；SPEC-017 §11.3 新增「PR #1 動工前 PoC gating」明示 PoC 結果未回填即拒絕 merge PR #1；SPEC-010 v0.2.1 → v0.2.2、SPEC-017 v0.2.5 → v0.2.6。(3) ADR README 索引補 ADR-0005。本輪後**所有動工前清單已 closed**：兩條紅線清除（R-1 / R-2）+ 跨檔一致性（H-1~H-6）+ 跨檔治理（M-1, M-3, multi-sig 中央表）+ 12 個技術風險點（T-1~T-12）+ ADR-0001 §6 patch + ADR-0005 retroactive + ADR-0006 INV-6 + PoC plan 文件化。Codex 拿到 SPEC + ADR + PoC plan 即可依 SPEC-017 §11.3 跑 PoC，PoC 通過後依 §11.2 開 PR #1。 |
| 2026-05-07 | v0.2.7 | Tech Lead | **動工前剩餘待辦清除 round 5**（ADR governance + T-1 ~ T-8 落地）：(1) **ADR-0001 v1.0.2**：§6「唯一例外」carve-out 撤除（連動 R-2），補 `probability_symmetry` / `auto_hedge` 禁字；(2) **ADR-0006 新增（Accepted）**：擴展 ADR-0003 §5 INV 集合 INV-1 ~ INV-5 → INV-1 ~ INV-6，正式 governance 追認 INV-2/INV-4 含 Frozen 子科目（v0.1.1 SPEC drift 補正），新增 INV-6 outflow whitelist（連動 R-1）+ plpgsql trigger template + application/DB/CI 三層保護；(3) **ADR-0003 v1.0.3**：§5 標 partial superseded by ADR-0006；(4) **ADR README**：索引補 ADR-0006，ADR-0001 / ADR-0003 狀態加註修訂；(5) **T-1 service 拓撲**：SPEC-014 §4.1 advisory 建議 MVP 7-binary 合併部署（保留 12 service crate 結構），DAU > 5k 後再拆；(6) **T-2 RPO 拆兩層**：SPEC-008 §2 RTO/RPO 表 RPO 拆「region-local（synchronous standby + WAL fdatasync + Kafka acks=all → RPO=0）」+「cross-region（async streaming → RPO ≤ 5s）」；新增 §2.1/§2.2 三重保證 + SLA 邊界；SPEC-008 v0.1.0 → v0.2.0；(7) **T-3 partition 策略修正**：SPEC-002 §12 OQ #2 改為「Phase 1 從 day 1 即 monthly partition by `created_at` (pg_partman) + BRIN + cold detach」，撤銷原「不分區後 migrate」設計；(8) **T-4 hostNetwork 路由**：SPEC-003 §17.2 落地 etcd-driven consistent hash routing（24 virtual nodes）+ 5 種觸發場景 + 跨服務銜接 + MVP-Scale-up-Scale-out 演進；(9) **T-5 最終一致性窗口**：SPEC-002 §9.3 明示 ~100ms eventual consistency 語意 + 前端 optimistic UI + 後端 `?at_seq=N` / `?consistent=true` API + OTel 監控；(10) **T-6 Thread model 分流**：SPEC-003 §17.3 落地「熱市場（前 5%）獨佔 node + current_thread + core pinning」vs「冷市場 multiplex + multi-thread + work-stealing」+ 動態升降級 + 兩 binary 拆分；(11) **T-7 Mutation 門檻分階段**：SPEC-017 §3.3 R-4 改為「MVP 60% 起步、每季 +5%、6 個月達 75%」；(12) **T-8 CI runner**：SPEC-017 §11.2.10 預設改 `ubuntu-latest-4-cores`（< 15 min cold cache），standard 2-core fallback。版本 bump：SPEC-002 v0.1.6 → v0.1.7、SPEC-003 v0.2.2 → v0.2.3、SPEC-008 v0.1.0 → v0.2.0、SPEC-014 v0.2.3 → v0.2.4、SPEC-017 v0.2.4 → v0.2.5；ADR-0001 v1.0.1 → v1.0.2、ADR-0003 v1.0.2 → v1.0.3、ADR-0006 首版 v1.0 Accepted。 |
| 2026-05-07 | v0.2.6 | Tech Lead | **技術可實現性收斂 round 4**（T-9 ~ T-12 + H-3 ~ H-6 + 新增 latency budget SPEC）：(1) **T-12 Settlement batch**：SPEC-004 §7.4 batch size 1000 → 500 + 新增 §7.5 強制 off-peak 排程（UTC 18-22 / 台北 02-06），降低 transaction window 與撮合 ledger worker lock contention；SPEC-004 v0.1.1 → v0.1.2。(2) **T-11 SLO 表補漏 + 新增 §6.4 Latency Budget**：SPEC-007 §6.1 補 wallet-ledger / market-service 兩個 P0 service SLO；Matching Engine SLO 拆「引擎內部 < 5ms」+「下單端到端 < 30ms」雙層；新增 §6.4 跨服務 Latency Budget（11 hop × P99 預算分解 + OTel span 對照 + 違反處理 + budget 變更管控），明示「各 service P99 達標但加總超標」之 SDD 防呆設計；SPEC-007 v0.1.0 → v0.2.0。(3) **T-10 Cloudflare Edge runtime PoC**：SPEC-014 新增 §3.2.1 列五個動工前必驗證場景（KYC iframe / WS fan-out / Server Actions / fetch / env binding），含已知限制與退路選項（Node.js runtime / 後端 BFF）；SPEC-014 v0.2.2 → v0.2.3。(4) **T-9 EdDSA mobile PoC**：SPEC-010 §3.1.2 補 LIFF SDK / mobile / browser 之 Ed25519 相容性 PoC 表，含 RS256 後備策略；SPEC-010 v0.2.0 → v0.2.1。(5) **H-3 CI gate 對齊**：SPEC-013 §3.1 改為「SPEC-017 §8 之超集」明示繼承全部 bootstrap gate（pnpm/cargo/sqlx/buf/clippy/nextest/deny/geiger/sentinel-scan）+ prod gate（SAST/SCA/image scan/IaC/secret/migration/E2E）；§3.2 必過 gate 表 19 列每列標來源；nightly 加 cargo-mutants / macos-latest / terraform plan diff；prod approval cross-ref MS-12。(6) **H-4 ORM → sqlx**：SPEC-010 §3.2 OWASP A03 由「ORM + parameterized」修為「`sqlx::query!` 編譯時檢查 SQL + parameterized binding」，對齊 SPEC-014 §3.1 不引入 ORM 決議。(7) **H-5 Chargeback 門檻統一**：SPEC-016 §6.1（≥ 1）/ SPEC-010 §6.2（> 1 → ≥ 1）/ SPEC-002 §8.1（補上門檻）三處對齊為「累計 chargeback **≥ 1** 永久封號 + 黑名單」；SPEC-002 v0.1.5 → v0.1.6、SPEC-010 v0.2.0 → v0.2.1、SPEC-016 v0.1.0 → v0.1.1。(8) **H-6 Coverage 分層**：SPEC-013 §3.3 新增覆蓋率分層表（ledger/撮合 ≥ 95% / 結算 ≥ 90% / 安全敏感 ≥ 85% / 其他 ≥ 80% / web ≥ 70% / mocks 不檢查），消除 SPEC-013（80%）vs SPEC-003 §16.1（100%）vs SPEC-003 §18（≥ 95%）三處內部矛盾；SPEC-003 §16.1 對齊修為「≥ 95% baseline + 100% stretch」；SPEC-013 v0.1.0 → v0.2.0、SPEC-013「依賴」加 SPEC-017。 |
| 2026-05-07 | v0.2.5 | Tech Lead | **R-1 / R-2 兩條合規紅線清除**（業主 2026-05-07 拍板）：(1) **R-1 點數回兌紅線**：SPEC-011 §6 整節重寫為 4 子節（B2B 簽約獨立法律關係 / 雙身份隔離 / 帳本層 INV-6 強制 / 會計科目分離）；§5.2 / §7.1 同步修正誤導描述；SPEC-002 §4.2 Chart of Accounts 補 `Liability.AccountsPayable.{partner_id}` / `Expense.AffiliatePartnerFee` / `Bank.OperatingAccount.{currency}`；SPEC-002 §5 新增 INV-6（`Player.Wallet.*` outflow 白名單，DB CONSTRAINT TRIGGER 強制 + CI sentinel-scan grep gate），ADR-0006 待落地擴展 ADR-0003 §5 INV 集合；SPEC-002 v0.1.4 → v0.1.5、SPEC-011 v0.1.0 → v0.2.0、SPEC-011「依賴」清單加 SPEC-010。(2) **R-2 平台下場紅線**：SPEC-009 §5 「冷啟動補貼」整段移除（保留 placeholder 章節說明移除理由與替代方案 — DMM rebate 放大）；SPEC-009 §9 OQ #5 標 Resolved；SPEC-003 §12.1.1 sentinel `system.platform_announcement` (uid=4) 移除並加入 `RESERVED_REMOVED_USER_IDS` 防重用集合 + 兩個 unit test；SPEC-003 §12.1.4「例外情況」整段刪除（改為「**無**」）；SPEC-003 v0.2.1 → v0.2.2、SPEC-009 v0.2.0 → v0.2.1。(3) 本檔 §5 新增 R-1 / R-2 governance 條目（採 🔴 → ✅ 圖示保留 deal-breaker 痕跡供未來 audit）；§5.1 圖示說明補新狀態。(4) **後續落地**：ADR-0001 §6 「唯一例外」描述待新 ADR 標記為 superseded；ADR-0006 待業主審閱後落地。 |
| 2026-05-07 | v0.2.4 | Tech Lead | 動工前一致性 sweep round 3：(1) **H-1 Go → Rust 殘留清理**：SPEC-003 §3.1 Market struct / §3.2 Outcome enum / §12.1.1-3 sentinel 與 CI grep / §14.2 金額計算 / §15 orderbook 範例 / §16 測試策略（`cargo nextest` 取代 `goleak`、`proptest` 取代 `gopter`，新增 §16.6 mutation testing 對齊 R-4）；SPEC-004 §4.2 OracleSubmission struct；SPEC-009 §7 DMM session 程式碼改 `tokio` + `tokio-rustls` + `thiserror`；SPEC-009 v0.1.0 → v0.2.0、SPEC-003 v0.2.0 → v0.2.1、SPEC-004 v0.1.0 → v0.1.1。SPEC-003 §6 ~ §11 之演算法 pseudo-code 維持 Go 風格（已由 §6 開頭註記為 pseudo-code，本次不動）。(2) **H-2 Rust 1.78 → 1.85 化石清理**：SPEC-014 §3.1 表格、SPEC-017 §2 file tree 註解、SPEC-017 §8 CI workflow `toolchain: '1.78'` → `'1.85'`（edition 2024）；SPEC-014 v0.2.1 → v0.2.2、SPEC-017 v0.2.3 → v0.2.4。(3) **M-1 Hedge 政策 Resolved（Q-7）**：新增本檔 §5 Q-7 為「允許 hedge」（對齊 Kalshi / Polymarket 業界做法）；SPEC-002 §4.4.4 NOTE 3 加 cross-ref；SPEC-015 §8.1 刪 hedge 禁令並改為 hedge 允許說明 + post-trade 偵測引用；SPEC-003 §12 加允許註記，明示**不**加 `HEDGE_NOT_ALLOWED` pre-trade（撮合 hot path 無新增 1 hop）；SPEC-002 v0.1.3 → v0.1.4、SPEC-015 v0.1.0 → v0.1.1。(4) **M-3 Multi-sig 治理中央表**：SPEC-010 新增 §9.3（17 條規則 MS-1 ~ MS-17 + 已 Resolved 不入表決策 + Admin UI 實作要求 + 為何集中），原各 SPEC（002 / 004 / 008 / 012 / 013 / 015 / 016 / 017）之簽核要求皆改以「呼應 SPEC-010 §9.3 MS-N」方式引用，避免 admin / Compliance UI 流程分裂；SPEC-010 v0.1.0 → v0.2.0；SPEC-010「被依賴」清單擴增為 9 份反映其 governance 中央表角色。 |
| 2026-05-06 | v0.2.3 | Codex | SPEC-005 工程實作完成並驗證通過；標記 KYC/AML 整合可視為完成，涵蓋 `kyc-service`、`mock-sumsub`、`packages/ts-sdk`、`apps/web` 與 seed / migration 流程。 |
| 2026-05-05 | v0.2.2 | Tech Lead | SPEC-017 §11 擴充：拆 §11.1（整體 Definition of Done）+ §11.2（Codex PR #1 acceptance checklist），含 10 個子節（repo 根結構 / Rust workspace / Protobuf / Frontend / docker-compose / env / CI / tools / 文件 / 規模預期）約 50 項勾選項，明列 PR diff 規模 5-9k 行、commit 8-15、CI cold cache < 25 min 與 reject 紅線。SPEC-017 v0.2.1 → v0.2.2。 |
| 2026-05-05 | v0.2.1 | Tech Lead | Rust 工具鏈 6 項硬性決議拍板（R-1..R-6）：(1) **R-1 TLS**：rustls 為預設（sqlx / fred / reqwest / tonic / opentelemetry 全 rustls-tls），rdkafka 例外沿用 librdkafka OpenSSL；(2) **R-2 Edition**：採 Rust edition **2024** + MSRV **1.85**；(3) **R-3 Runtime**：撮合引擎 current_thread + per-market `LocalSet` + `core_affinity` thread pinning；其餘 11 個 service 走 multi-thread runtime；(4) **R-4 Mutation testing**：cargo-mutants 限縮 `crates/sdk/ledger` + `services/{wallet-ledger-service,matching-engine}` 核心邏輯，nightly CI cron，caught rate ≥ 75%；(5) **R-5 Unsafe**：workspace 全域 `#![forbid(unsafe_code)]`，例外須 ADR + 4-eye review，CI 跑 cargo geiger；(6) **R-6 sqlx offline**：`.sqlx/` commit 進 git，CI 跑 `cargo sqlx prepare --check`。落入 SPEC-014 §3.1 / SPEC-017 §3.3 / §4.2 / §6.1.1 / §8 / §8.2，及 .gitignore。SPEC-014 v0.2.0 → v0.2.1，SPEC-017 v0.2.0 → v0.2.1。 |
| 2026-05-05 | v0.2.0 | Tech Lead | **後端語言由 Go 切換至 Rust（重大）**：(1) SPEC-014 v0.2.0 改寫 §3.1 後端技術棧（Rust 1.78+ / axum / tonic / sqlx / tokio / sccache / cargo-nextest / rdkafka / fred / tracing / dotenvy / ulid），§4 service 表全標 Rust，§5 monorepo 結構新增 `crates/sdk/` + `crates/proto/`、移除 `packages/go-sdk/`，§6 / §8 / §9 / §10 / §12 / §14 同步；§14 OQ #5「撮合切 Rust」隨之 Resolved；(2) SPEC-017 v0.2.0：§2 file tree、§3 toolchain（Rust 1.78+ / clippy / rustfmt / sqlx-cli + sccache + cargo-nextest + cargo-chef + cargo-deny）、§4.2 Cargo workspace 取代 go.work、§4.4 prost/tonic 取代 protoc-gen-go、§4.5 Makefile cargo 命令、§6.1 Rust axum stub 範例、§8 CI 改 dtolnay/rust-toolchain + sccache + Swatinem/rust-cache、§11 / §12 同步；(3) SPEC-003 v0.2.0：§1 in-process memory 改 Rust、§4.3 `OrderID = [u8; 16]` Rust canonical、§5 OrderBook 改 BTreeMap + VecDeque、§6 開頭加 pseudocode 註記、§19 OQ #3 落地動作對齊 ulid crate；(4) SPEC-002 v0.1.3：§3.2 內部型別 int64 → i64；(5) SPEC-013 / 016 / README / ADR-0004 / .gitignore / packages/proto/README + common.proto / CLAUDE.md 同步；(6) packages/proto/v1/common.proto 之 `OrderID` 由 uint64 改為 `bytes`（16 bytes ULID）對齊 SPEC-003 §19。 |
| 2026-05-05 | v0.1.7 | Tech Lead | Codex 動工前 OQ 收尾 round 2：(1) SPEC-017 §12 OQ #3 mock servers 同 monorepo Resolved（含 production binary 隔離 build tag）；(2) SPEC-002 §12 OQ #2 `ledger_entries` Phase 1 不分區，明列分區觸發門檻與 monthly partition 為未來方案；(3) SPEC-003 §19 OQ #3 訂單 ID 採 ULID（保留 per-market `seq uint64` 為撮合 FIFO 排序），§4.3 加 TODO 標記 PR #1 改 [16]byte；(4) SPEC-016 §7 Phase 2 多幣別觸發條件由「Q-3 拍板」改為業務驅動（Op Co. 國際版本 / FX 對沖），明示**目前無啟用計畫**；SPEC-002 §3.1 國際化說明同步收斂。 |
| 2026-05-05 | v0.1.6 | Tech Lead | Codex 動工前 deal-breaker 收尾：(1) Q-3 點數計價 1 點 = TWD 1 由暫定改為 ✅ Resolved（業主拍板），SPEC-002 §3.1 同步；(2) Q-6 數據主權暫定 Cloudflare R2 ✅ Resolved；(3) SPEC-005 §2.1 釘住 KYC tier canonical enum `TIER_0/1/2/3`，SPEC-017 §5.2 同步；(4) SPEC-017 §12 OQ #1 / #2 / #5 全部 Resolved（commit gen/、env_file、Linux PR gate + macOS nightly）；(5) SPEC-002 §12 OQ #1 ledger 拆獨立 service Resolved，§4.2.1 新增 `Player.Frozen.{user_id}` 子科目，INV-2 同步擴展；(6) SPEC-002 §12 OQ #4 自我排除錢包處理 Resolved。 |
| 2026-05-05 | v0.1.5 | Tech Lead | Codex 動工前置補強：(1) 新增 SPEC-017 monorepo bootstrap & local dev（P0）；(2) 建立 packages/proto/v1/ 共 6 份 .proto（errors / common / ledger / engine / market / kyc / payment）；(3) SPEC-002 §4.4.3 trade entry 改寫對齊 §4.4.4 不對稱 escrow，移除「兩邊皆鎖 6,500」之簡化；(4) SPEC-015 §6 補 outcomes / oracle_submissions / disputes / dispute_resolutions / positions 五張 schema，SPEC-004 §6.3 改為呼應引用；(5) SPEC-016 §5.5 補通路 webhook payload 對映表（綠界 / Stripe / 藍新 / PayMongo / MoMo / VNPay 6 家）；(6) SPEC-010 §3.1 補 JWT claims schema、refresh token rotation、reuse detection、step-up MFA 細節。 |
| 2026-05-05 | v0.1.4 | Tech Lead | 第二輪交叉一致性 review（純技術視角，合規由業主自治）：(1) SPEC-005 §2.4 對 SPEC-001 §3.1 之 dead link 改為指 SPEC-001 §2.1；(2) SPEC-015 §11 7 年保留改為呼應 SPEC-001 §1.1 ③；(3) ADR-0002 多處對 SPEC-001 §2 / §2.2 / §8.3 / §10、SPEC-002 §2.1 之 dead link 全部修正並補 v1.0.2 Revision History；(4) 補回 SPEC-002 §3.2 內部精度（純技術整數化規範）；(5) SPEC-016 §7 跨幣別整節重寫，明確 Phase 1 單幣別 / Phase 2 多幣別保留設計，移除 `Bank.TrustAccount.USD` 舉例；(6) SPEC-002 §12 Open Questions 補齊編號 1/2 並收斂跨檔議題引用。 |
| 2026-05-05 | v0.1.3 | Tech Lead | 依 SPEC-001 v0.1.1 精簡後章節重整索引：更新 SPEC-001 標題、摘要、狀態版本；Geo-block 全域描述改以 SPEC-001 §2 為準；跨檔決策追蹤表中 SPEC-001 章節引用改為現行 §2 / §3.1 / §4.1，移除對已刪除公司結構與司法管轄區分級表的硬引用。 |
| 2026-05-05 | v0.1.2 | Tech Lead | 完成首輪交叉一致性 review：(1) 修 SPEC-002 §5 INV 編號對齊 ADR-0003 §5；(2) 修 PRD.md dead link；(3) HK/SG geo-block 統一為保守 🔴；(4) 修 SPEC-002 §4.4.4 零金額結算 entry 衝突；(5) ADR-0002 / ADR-0003 §11 改名為「Pre-Execution Validation Items」；(6) §5 改為跨檔決策追蹤表（含 Resolved-by 欄位）；(7) SPEC-005 補 declared_monthly_income 並對齊 SPEC-006 §4.1；(8) SPEC-005 §2.4 補首發 5 國 Tier 1 證件驗證；(9) 新增 SPEC-015 市場生命週期；(10) 新增 SPEC-016 支付通路整合；(11) SPEC-003 §12.1 補 PLATFORM_SYSTEM_USER_IDS sentinel 定義；(12) SPEC-015 / SPEC-016 加入 P0 索引。 |
| 2026-05-05 | v0.1.1 | Tech Lead | 專案英文代號定案：propheticaNet（暫定）→ prophetica；同步刪除 legacy PRD.md（內容與 SPEC-000 / SPEC-001 多處衝突）；連帶 bump SPEC-001 / SPEC-005 / SPEC-014 至 v0.1.1（修正內文專案名稱）；ADR-0001 / ADR-0002 同步修名但依 ADR 慣例不變動 Status / Date。 |
| 2026-05-05 | v0.1.0 | Tech Lead | 初版建立，14 份 SPEC 全數草稿完成 |

## 5. 跨檔決策追蹤表（Cross-cutting Decision Tracker）

> 本表為跨多份 SPEC / ADR 的全域決策點之**單一真相來源**。任何議題一旦被某 SPEC / ADR 決定，**Status** 改為 `Resolved` 並填入 `Resolved by` 欄位；其他文件的 Open Questions 不重複記錄此議題。

| ID | 議題 | 影響範圍 | Status | Resolved by | 待動作 |
|----|------|---------|--------|------------|--------|
| Q-1 | 控股公司司法地：BVI vs Cayman vs Delaware | ADR-0002 | ✅ Resolved | [ADR-0002 §4.4](../docs/architecture/decisions/ADR-0002-three-company-corporate-structure.md) | BVI；未來 IPO 路徑前重組為 Cayman/Delaware |
| Q-2 | Op Co. 執照地：Malta MGA / Curaçao / PAGCOR / Isle of Man | ADR-0002 §11 V-1 | 🟡 Pending | — | 動工前由執照地律師擇一，影響 Op Co. 設立地 |
| Q-3 | 點數計價單位：1 點 = TWD 1 vs USD 0.01 vs 抽象單位 | SPEC-002 §3.1 / ADR-0003 §11 V-5 | ✅ Resolved | [SPEC-002 §3.1](./SPEC-002-funds-safety.md) | 1 點 = TWD 1（內部記帳基準幣別恆為 TWD，多幣別走 SPEC-016 §7 Phase 2）|
| Q-4 | 入金通路供應商：綠界 / 藍新 / 紅陽 / Stripe；MCC 接受度 | SPEC-001 §2.1 / SPEC-016 | 🟡 Pending | [SPEC-016 §3.2](./SPEC-016-payment-gateway.md) 列驗證項 | 簽通路合約前完成至少 3 家詢價，並對齊支付 Gateway BIN 檢查 |
| Q-5 | 未成年下限：18 歲 vs 20 歲 | SPEC-001 §3.1 / SPEC-006 §6 | ✅ Resolved | [SPEC-006 §6](./SPEC-006-responsible-gaming.md) | 18 歲（依台灣民法成年新制）|
| Q-6 | 數據主權：Cloudflare R2（EU / 全球）vs Linode Object Storage (Tokyo) vs AWS S3 (Singapore) | SPEC-001 §4.1 / SPEC-005 §5.1 | ✅ Resolved（暫定）| 業主決議 2026-05-05 | KYC 證件 / WORM hash chain / 影像類資產一律存 **Cloudflare R2**（區域選擇待執照地律師最終確認，SPEC-001 §4.1 預設 EU bucket + Object Lock）；撮合事件流冷備走 SPEC-008 §6.1 雙寫策略 |
| Q-7 | Hedge 政策：同帳戶在同一市場是否允許同時持 YES + NO？ | SPEC-002 §4.4.4 / SPEC-003 §12 / SPEC-015 §8.1 | ✅ Resolved | Tech Lead 2026-05-07 | **允許 hedge**：保留 SPEC-002 §4.4.4 NOTE 3 之逐 `position_id` 結算邏輯；**移除** SPEC-015 §8.1 hedge 禁令該行；SPEC-003 §12 風控前置層**不**加 `HEDGE_NOT_ALLOWED` 檢查（撮合 hot path 無新增 1 hop）；同帳戶自我對沖之刷量行為由 SPEC-010 §4.2.2 wash trading post-trade 偵測攔截。對齊 Kalshi / Polymarket 業界做法 |
| **R-1** | 點數回兌紅線：是否允許「玩家錢包點數 → B2B 應付帳款 / 現金」之任何技術路徑？ | SPEC-001 §1.1 ① / SPEC-002 §4.2 §5 / SPEC-011 §6 | 🔴 → ✅ Resolved | 業主決議 2026-05-07 | **永不允許**：徹底清除 SPEC-011 §6 原「業務帳戶可選擇將點數獎勵轉為 B2B 合約應付」設計；§6 重寫為 4 子節（B2B 簽約獨立、雙身份隔離、帳本層 INV-6 強制、會計科目分離 `Expense.AffiliateCommission` vs `Expense.AffiliatePartnerFee`）；SPEC-002 §5 新增 INV-6 由 DB CONSTRAINT TRIGGER 強制 `Player.Wallet.*` outflow 目的地白名單；ADR-0006 待落地擴展 ADR-0003 §5 INV 集合為 INV-1 ~ INV-6 |
| **R-2** | 平台下場紅線：是否保留「平台公告錢包」冷啟動補貼之 feature flag / sentinel 例外？ | SPEC-001 §1.1 ② / SPEC-003 §12.1 / SPEC-009 §5 / ADR-0001 §6 | 🔴 → ✅ Resolved | 業主決議 2026-05-07 | **永不允許，徹底刪除**：SPEC-009 §5 「冷啟動補貼」整段移除（保留 placeholder 章節說明）；SPEC-003 §12.1.1 sentinel `system.platform_announcement` (uid=4) 移除並加入 `RESERVED_REMOVED_USER_IDS` 防重用；SPEC-003 §12.1.4「例外情況」整段刪除（改為「**無**」）；流動性不足走 SPEC-009 §3.3 DMM rebate 放大或市場延後上架；ADR-0001 §6 「唯一例外」描述待新 ADR 更新為「無例外」 |
| Q-8 | SPEC-011 落地前 8 個技術 gap（推薦碼演算法 / 親屬偵測子規則 / 達標 claw-back / 帳本流向 / chargeback 反沖 / sign convention / 用詞 / 獎勵上限）| SPEC-011 §3.2 §3.3 §4.1 §4.2 §4.4 §4.5 §4.6 §5.1 / SPEC-002 §5 INV-6 / SPEC-010 §4.2.1 | ✅ Resolved | [SPEC-011 v0.2.1](./SPEC-011-affiliate-commission.md) 2026-05-07 | **G-1** BLAKE3 + Crockford Base32 + 5 retries；**G-2** `affiliate_kinship_link` 子規則 6 特徵 + 雙 gate；**G-3** 延遲 30 天發放（從根源繞開 INV-6 clawback 衝突）；**G-4** 獨立每日批次 txn + `commission_accruals` staging；**G-5** chargeback 反沖：介紹獎勵 G-3 路徑無需 clawback、持續佣金自停、已付佣金不追回；**G-6** sign convention cross-ref SPEC-002 §4.3；**G-7**「推薦樹」→「推薦清單」；**G-8** 月度 20 / 終身 200 / 月度佣金 TWD 50k 上限。SPEC-002 §5 INV-6 **不**需新增 affiliate clawback carve-out。Codex 動工輸入完備 |

### 5.1 議題狀態圖示

| 圖示 | 含義 |
|------|------|
| ✅ Resolved | 已被某 SPEC / ADR 拍板 |
| 🟡 Pending | 待決定，但不阻塞動工 |
| 🔴 Deal-breaker | 阻塞 P0 收尾，必須在第一行 code 之前解決 |
| 🔴 → ✅ | 原為 Deal-breaker，現已 Resolved（保留 🔴 標記提示其重要性，方便日後 audit） |

### 5.2 維護規則

- 新增跨檔議題請寫成新一列；**不要**在個別 SPEC 的 Open Questions 重複記錄已在本表的議題
- 議題被拍板後，將相關 SPEC 的 Open Questions 條目刪除並改為「見 SPEC-000 §5 Q-N」
- ADR 體裁中已 Accepted 的 ADR，§11 採用 `Pre-Execution Validation Items` 命名（驗證項目而非待決議題），參見 ADR-0002 / ADR-0003
