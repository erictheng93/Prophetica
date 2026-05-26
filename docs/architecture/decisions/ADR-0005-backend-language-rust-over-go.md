# ADR-0005 — 後端語言由 Go 切換至 Rust（全平台）

| 欄位            | 內容                                                                                                                                                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Status**      | Accepted（**retroactive** — 決策實際發生於 2026-05-05 隨 SPEC-014 v0.2.0；本 ADR 於 2026-05-07 補做 governance 追認）                                                                                                                           |
| **Date**        | 2026-05-05（決策日）/ 2026-05-07（ADR 撰寫日）                                                                                                                                                                                                  |
| **Deciders**    | Founder, Tech Lead                                                                                                                                                                                                                              |
| **Consulted**   | 預定：撮合 / ledger 工程師（待招募）、Codex 開發代理；Anthropic Rust 社群參考資料                                                                                                                                                               |
| **Informed**    | 全工程團隊、Compliance Officer（編譯時間影響迭代）、QA                                                                                                                                                                                          |
| **Tags**        | backend-language · core-architecture · runtime · type-safety · matching-engine · ledger                                                                                                                                                         |
| **依賴 / 影響** | SPEC-014 §3.1 / §4 / §5 / §6 / §8 / §9 / §10 / §12 / §14（v0.2.0 隨同更新）；SPEC-017 §2 / §3 / §4 / §6 / §8 / §11 / §12（v0.2.0 隨同更新）；SPEC-002 §3.2（v0.1.3）；SPEC-003 整檔（v0.2.0）；ADR-0006（INV-6 trigger plpgsql 落地依賴本決策） |

---

## 1. Context and Problem Statement

PRD v0 與 SPEC-014 v0.1.x 原指定後端為 **Go 1.22+**。此選擇在 2026-05-05 之前的所有 SPEC（v0.1.0 ~ v0.1.7）均未變動 — Go 對於撮合 / ledger / WebSocket 等場景皆有合理生態支撐，且台灣工程師供給充足。

但於 2026-05-05 業主與 Tech Lead 重新評估後，決定**全平台切換為 Rust 1.85+ stable（edition 2024）**，理由聚焦於：

1. **撮合 hot path 對 GC pause 之零容忍**：SPEC-003 §11 規定撮合引擎內部 P99 < 5ms。Go GC 即便採 `GOGC=off` + `GODEBUG=gctrace=1` 調優，stop-the-world tail latency 在 high allocation rate 下仍會出現 1-3ms 抖動，吃掉 P99 budget 的 20-60%
2. **雙簿記不變式之型別系統強制**：SPEC-002 §5 INV-1 ~ INV-6 為「entries 借貸代數和為 0」「outflow 白名單」等不變式；Rust 之 newtype pattern + `Send + Sync` + lifetimes 可在編譯時阻擋大量會違反 INV 的程式碼路徑（如錯誤共享 ledger transaction handle、跨執行緒誤用未 commit 之 entry）；Go 同類保護需 runtime check + code review
3. **撮合 single-writer 不變式之型別系統表達**：SPEC-003 §7 之 per-market single-writer 在 Rust 下可由 `!Send` marker type + `LocalSet` 編譯時鎖死（`OrderBook` 不可跨執行緒移動）；Go 需依賴 channel 文化與 code review
4. **業界對標 Kalshi**：Kalshi 撮合引擎已知為 Rust 實作（公開技術部落格 2023）；招募有 Kalshi / Coinbase / IEX 經驗工程師時，Rust 對話成本低於 Go
5. **SPEC-014 v0.2.0 開始之 unsafe / TLS / mutation testing 治理（R-1 ~ R-6）需要 Rust 生態**：cargo-mutants / cargo-geiger / cargo-deny / sqlx compile-time SQL 等工具鏈在 Rust 成熟度遠高於 Go 同類工具

問題是：**切換後端語言是重大決策**，需要 ADR 正式 governance 化，但 SPEC-014 v0.2.0 變更日誌內已詳細記錄並隨同 ~10 份 SPEC 同步更新，未獨立寫 ADR。本 ADR 為**追溯性 governance 補正**，將該決策從「SPEC 變更日誌」升格至「架構決策層級」，便於未來監管 / 招募 / 新進工程師詢問時提供單一答覆來源。

## 2. Decision Drivers

依重要性遞減：

1. **撮合 / ledger hot path 之 P99 tail latency 不可被 GC 主導**：金融撮合系統最致命的失敗模式是「壓測 P50 達標但 P99 在尖峰被 GC pause 拉高」，Rust 之無 GC 設計從根本消除此風險
2. **不變式型別化（type-driven invariants）**：雙簿記 INV / 撮合 single-writer / point-no-cash-out 紅線等都能透過 Rust 型別系統（newtype / phantom data / `Send + Sync` / lifetimes）編譯時強制，降低 application bug 進入 production 機率
3. **依賴審計成本**：金融服務監管要求依賴授權清晰、CVE 響應快速；Rust 之 `cargo deny` + `cargo geiger` + `crates.io` 之 yanked 機制成熟度優於 Go modules
4. **長期維運**：撮合引擎一旦上線，重寫成本極高；前期多花 1-2 週寫 Rust 換取 5-10 年無 GC 抖動之 production，ROI 正向
5. **編譯時間（負面但可控）**：sccache + cargo-chef + Cargo workspace incremental build 可將 CI 暖快取下單服務 build 控在 < 90s，CI 完整 cold cache < 25 min（SPEC-017 v0.2.5 改 4-cores runner 後 < 15 min）
6. **Rust 學習曲線（負面但可控）**：CRUD-style service（user / kyc / notification）借助 axum + sqlx + `?` 運算子，程式量與 Go 相當；borrow checker 對非熱點服務之主要成本為前 2 週適應期

## 3. Considered Options

### Option A — 維持 Go 1.22+（PRD / SPEC-014 v0.1.x 原指定）

繼續以 Go 實作 12 個 service。撮合 hot path 透過 GC 調優 + sync.Pool + 預分配 buffer 降低 GC pressure。

### Option B — Rust hot path + Go CRUD 混搭

撮合 / ledger / settlement-worker 用 Rust；user / kyc / notification / referral 等 CRUD 用 Go。透過 protobuf / gRPC 跨語言溝通。

### Option C — 全 Rust（最終選擇）⭐

全平台 12 個 service 統一 Rust 1.85+。

### Option D — C++ for hot path

撮合 / ledger 用 C++（HFT 業界做法）；其他 service 用 Go 或 Rust。

## 4. Decision Outcome

**選擇 Option C：全 Rust（含 CRUD service）。**

### 4.1 為何選 C 而非 A（維持 Go）

| 比較面向                  | Go 1.22                                | Rust 1.85                                     |
| ------------------------- | -------------------------------------- | --------------------------------------------- |
| GC pause（撮合 hot path） | 1-3ms tail latency 抖動                | **無 GC**，tail latency 可預期                |
| 不變式型別化              | 需 runtime check + code review         | newtype / `Send+Sync` / lifetimes 編譯時強制  |
| 編譯時 SQL 檢查           | sqlc（separate codegen step）          | sqlx `query!` macro（一體化 + offline cache） |
| Mutation testing          | go-mutesting（社群 maintain，limited） | cargo-mutants（Rust Foundation backed）       |
| Unsafe 監測               | go vet（基本）                         | cargo geiger（細粒度統計）                    |
| 業界對標撮合              | 較少（多數金融服務用 C++ / Java）      | Kalshi / Polkadot / Solana validator          |
| 編譯時間                  | < 30s incremental                      | < 90s incremental（with sccache）             |
| 招募成本（台灣）          | 充足（普及度高）                       | 中等（需培訓 + pair programming）             |
| 學習曲線                  | 1-2 週                                 | 4-6 週（borrow checker）                      |

決定性因素：**GC pause 對撮合 P99 之破壞不可接受**。即便 Go 1.22 GC 相比 1.18 已大幅改善（pacer rewrite、background mark），STW phase 在 high allocation 下仍非零；對於撮合引擎這種「每筆訂單都被計時」的場景，這個風險無法接受。

### 4.2 為何選 C 而非 B（Rust + Go 混搭）

| 議題              | 影響                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------ |
| 雙語言 SDK        | observability / errors / proto stub / ledger client 要寫兩套，維運成本高於單語言收益 |
| 雙語言 OTel 配置  | 兩套 SDK 版本對齊、attribute 命名一致性、span propagation 需額外 review              |
| 招募 / onboarding | 工程師需熟悉兩個生態、兩套 build tool、兩個 IDE 配置                                 |
| Code review pool  | Rust review 與 Go review 不通用，PR queue 容易堵塞                                   |
| 部署              | 兩套 base image / 兩套 cargo-chef vs go-build cache 策略                             |

混搭方案在小團隊（< 20 人）下管理複雜度大於收益。Option B 在大型企業（> 100 人）有合理性（如 Discord 用 Rust + Elixir），但本專案規模不適用。

### 4.3 為何選 C 而非 D（C++ for hot path）

C++ 在 HFT 業界成熟，但：

- **記憶體安全**：C++ memory safety 完全靠紀律與 sanitizer；Rust 是編譯時保證。對於金融系統 zero tolerance 之 memory bug，Rust 安全網更強
- **依賴管理**：CMake / Conan / vcpkg 生態破碎；Cargo 一體化
- **招募**：C++ 撮合工程師供給少且昂貴，Rust 雖也緊俏但學習曲線較友善（無 manual memory management）
- **與 axum / tonic / sqlx 生態整合**：Rust 之非熱點 service 工具鏈成熟；C++ 同類（如 cppcoro / uvw / sqlpp11）較不主流

D 否決，但採納其**設計理念**：撮合 hot path 之 lock-free / cache-aware 設計可借鏡 C++ HFT 模式（如 LMAX Disruptor），SPEC-003 §5 之 BTreeMap + VecDeque + tombstone cancel 即受此影響。

## 5. Consequences

### 5.1 Positive

1. **撮合 P99 budget 寬鬆**：無 GC pause，撮合引擎內部 P99 < 5ms 達成路徑直接（不需 GC 調優、不需 sync.Pool 巧思）
2. **不變式 compile-time enforcement**：SPEC-002 INV-1 ~ INV-6 / SPEC-003 single-writer / 銅幣出口紅線可透過 Rust 型別系統部分 / 大幅編譯時鎖死，降低 production bug 機率
3. **單語言 SDK / observability**：12 個 service 共用 `crates/sdk`（observability / errors / config / ledger client），維運成本最小化
4. **業界 Rust 生態正面循環**：sqlx / axum / tonic / fred / rdkafka 等核心套件成熟度高；新工具（如 cargo-mutants）採用率高
5. **長期 production stability**：Rust 程式上線後，crash / memory leak / data race 機率顯著低於 Go（Rust 之 fearless concurrency 設計）
6. **招募差異化**：「我們是 Rust + 金融撮合」對特定族群（系統工程師、HFT 退役、Kalshi 校友）有吸引力，可避開「又一個 Go 後端」之招募紅海

### 5.2 Negative

1. **早期開發速度下降 30-40%**：前 4-6 週工程師適應 borrow checker，PR review 額外時間，pair programming 成本
2. **招募池較窄**：台灣 Rust 工程師供給遠少於 Go；可能需從 Go / C++ 工程師培訓
3. **編譯時間顯著高於 Go**：cold build 12 services × `cargo build` 在 4-cores runner 下 ~12-15 min（Go 同等規模 ~3-5 min）
4. **CRUD service 部分過度設計**：user-service 之 CRUD 用 Rust 並無顯著技術優勢，僅為「單語言一致性」付出 borrow checker 成本
5. **Macro debugging 痛點**：sqlx `query!` / serde derive macro 出錯時 error message 對新手不友善
6. **第三方 SaaS Rust SDK 缺口**：KYC provider / Sift / Sardine 主推 Go / TypeScript / Python SDK，Rust 端常需用 reqwest 自行包 REST API

### 5.3 Mitigation Plan

針對 Negative #1 / #2：

- 前 4 週 mandatory pair programming，撮合 / ledger 兩名 senior 工程師輪流帶；定期讀書會（async-book、The Rustonomicon hot-path 章節）
- Tech Lead 親自 review 前 50 個 Rust PR，著重 unwrap / panic / unsafe / lifetime mistake
- 招募策略：優先找有 C++ 經驗者（borrow checker 適應快），其次 Go 經驗者，避免「我願學 Rust」但無系統程式背景之候選人

針對 Negative #3：

- CI runner 採 `ubuntu-latest-4-cores`（SPEC-017 v0.2.5 / T-8）
- sccache 共享快取（s3-backed）跨 PR 重用編譯產物
- cargo-chef 分層 Docker build cache
- 開發者本機 cargo-watch + 增量編譯
- Production binary 採 `lto = "thin"` + `codegen-units = 1` 一次性犧牲時間換 runtime 性能

針對 Negative #4：

- 接受此 trade-off；SPEC-014 §3.1 已明示「全 Rust 而非混搭」之決策理由（單語言內部 RPC 維運成本 > CRUD borrow checker 成本）

針對 Negative #5：

- 建立內部 cheat sheet：常見 sqlx / serde error message → 解法對照表
- VS Code 配置 rust-analyzer + Even Better TOML + crates 等 extension（onboarding 文件涵蓋）

針對 Negative #6：

- SaaS REST API 用 reqwest + serde 自行包；webhook receiver 在 axum middleware 統一處理（SPEC-016 §5.5.4 Driver trait 之設計理念）
- 必要時為高頻 SaaS（KYC provider / Stripe）寫一份 thin wrapper crate 放 `crates/sdk/saas-clients/`

## 6. Compliance & Legal Boundary

本決策本身無直接合規影響，但連動以下技術紅線：

- **R-5 unsafe 全域 forbid**（SPEC-014 §3.1 / SPEC-017 §3.3）：workspace 預設 `#![forbid(unsafe_code)]`，例外須 ADR + 4-eye review + cargo geiger 監測；金融系統不應有「靠 unsafe 提速」之路徑（呼應 SPEC-010 §9.3 MS-11）
- **INV-6 plpgsql trigger（ADR-0006）依賴 sqlx 編譯時 SQL**：DB 層 invariant 在 Rust 端透過 sqlx `query!` macro 編譯時驗證 SQL 對 schema 之相容性；切換到 Go 會失去此保護

## 7. Validation Criteria（Pre-Execution Validation Items）

> ADR README §修訂規則：Accepted 狀態之 ADR 此節為驗證項目（非待決議題）。

本決策已落地於 SPEC-014 v0.2.0 / SPEC-017 v0.2.0；以下為持續驗證項目：

- [ ] Codex PR #1 之 12 個 service crate 全部以 Rust + tokio + axum 實作（已寫入 SPEC-017 §11.2.2）
- [ ] CI cold cache build < 15 min（4-cores runner，SPEC-017 §11.2.10）
- [ ] 撮合引擎壓測 P99 < 5ms（SPEC-003 §11，無 GC pause 干擾）
- [ ] cargo-mutants ledger / 撮合 hot path caught rate 階段性達標（SPEC-017 §3.3 R-4 / T-7）
- [ ] cargo-geiger 全 workspace unsafe 行數 = 0（除明確 ADR 化之例外）
- [ ] sqlx offline cache `.sqlx/` 隨 schema migration 同步 commit（SPEC-017 §3.3 R-6）

## 8. References

### 8.1 內部文件

- [SPEC-014 §3.1](../../../specs/SPEC-014-tech-architecture.html) — 技術棧定版（Rust 1.85+）
- [SPEC-014 §3.1 R-1 ~ R-6](../../../specs/SPEC-014-tech-architecture.html) — Rust 工具鏈硬性決議
- [SPEC-017 §3.3](../../../specs/SPEC-017-monorepo-bootstrap.html) — Rust 工具鏈硬性決議落地
- [SPEC-003 §6 開頭註記](../../../specs/SPEC-003-matching-engine.html) — pseudo-code / canonical Rust 之邊界說明
- [ADR-0001](./ADR-0001-order-book-with-dmm-over-lmsr.md) — Order Book + DMM（撮合架構決策，本 ADR 為其語言選擇之下游）
- [ADR-0003](./ADR-0003-double-entry-ledger.md) — 雙簿記帳本（INV 集合，依賴 Rust 型別系統強制）
- [ADR-0006](./ADR-0006-ledger-invariant-set-extension-inv6.md) — INV-6 落地（plpgsql trigger 與 Rust application validate 雙層保護）

### 8.2 外部參考

- Kalshi Engineering Blog — "How Kalshi's Matching Engine Works"（2023，Rust 撮合架構公開）
- Stjepan Glavina — "Tasks of Their Own: How `tokio::task::LocalSet` Pins Work to a Single Thread"
- Niko Matsakis — "Async Rust: A Year in Review"（2024，async ecosystem maturity）
- Cliff Click — "The Garbage Collection Handbook" Ch. 18（GC tail latency 對 trading 系統影響）
- LMAX Disruptor White Paper（高吞吐 lock-free queue 設計，Rust 撮合借鑑）

## 9. Notes for Codex / Future Engineers

當你在任一 Rust service crate 上動手時：

1. **絕不**使用 `unwrap()` / `expect()` 在 production code path（hot path 加碼為 clippy `-D unwrap_used`）；錯誤一律 `Result<T, E>` + `?` 運算子向上 propagate
2. **絕不**在撮合 / ledger crate 引入 unsafe block，除非有對應 ADR 與 4-eye review
3. **絕不**在 hot path 呼叫 blocking syscall（DB / 檔案 IO）；用 `spawn_blocking` 隔離；CI 加 lint rule 偵測
4. SQL 一律用 `sqlx::query!` 編譯時驗證；若需 dynamic SQL（極少數），用 `sqlx::QueryBuilder`，**永不** string concatenation
5. 跨 service RPC 一律 protobuf + tonic；REST 僅用於對玩家邊緣（api-gateway）
6. 若你認為某 service 應該回到 Go（例如「這個 CRUD 用 Rust 太慢」）：**寫一份 ADR-XXXX 標記本 ADR 為 partial superseded，附上具體 latency / 開發成本量化證據**——不要直接改 Cargo.toml

## 10. Revision History

| 日期       | 版本 | 作者      | 變更                                                                                                                                                                                                                                                                           |
| ---------- | ---- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 2026-05-07 | v1.0 | Tech Lead | 首版 Accepted（**retroactive**）：將 2026-05-05 SPEC-014 v0.2.0 之「後端 Go → Rust」切換決策補做 ADR governance 化；說明 driver / options / consequences / mitigation；補 SPEC-014 v0.2.0 變更日誌未涵蓋之決策論證細節（特別是「為何全 Rust 而非混搭」「為何 Rust 而非 C++」） |
