# Pre-Flight Proof-of-Concept Plans

> **公開 Beta 前必跑之 PoC 清單**。Codex PR #1 / internal alpha 可在 live verification pending 時繼續，但公開 Beta、真實入金、真實下單與 production deployment mode finalization 之前，本文件所列之 PoC 必須由前端 / 全端工程師跑通並回填 PoC 結果至本文件 §3 / §4。任一 PoC 失敗 → 觸發退路選項並更新對應 SPEC 章節，再續進 release review。

| 文件版本 | v1.1                                                                                                                                                                                       |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 最後更新 | 2026-05-07                                                                                                                                                                                 |
| 維護人   | Tech Lead                                                                                                                                                                                  |
| 狀態     | 🟢 Desk Research Done · 🟡 Live Verification Pending（artifacts 已就緒於 [`eddsa/`](./eddsa/) 與 [`edge-runtime/`](./edge-runtime/)，工程師可即時跑 live verification 並回填 §3.5 / §4.4） |

---

## 1. PoC 執行原則

- **最小可重現案例**（Minimum Reproducible Example）優先：避免在完整 `apps/web` 內測，先建獨立 sandbox 驗證「核心相容性問題」是否存在
- **Acceptance criteria 必須可量化**：「能跑通」不是 acceptance；「具體 API call 在 X 環境下回傳 Y 結果」才是
- **退路路徑必須事先寫好**：若 PoC 失敗，怎麼改 SPEC、改哪個服務拓撲、改哪個依賴
- **PoC 結果回寫**：每個 PoC 跑完後，本文件對應 §3 / §4 需填入結果（pass / fail / 部分通過）+ 證據連結（commit SHA / branch / 螢幕截圖）

---

## 2. 連結至 SPEC

| PoC                                       | SPEC 章節                                                       | 阻擋之實作項目                                                                                                                   |
| ----------------------------------------- | --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| PoC-A: EdDSA mobile / LIFF SDK 相容性     | [SPEC-010 §3.1.2](../../specs/SPEC-010-security-antifraud.html) | JWT signing key 演算法公開 Beta 定版；LIFF production path                                                                       |
| PoC-B: Cloudflare Pages Edge runtime 限制 | [SPEC-014 §3.2.1](../../specs/SPEC-014-tech-architecture.html)  | apps/web 公開 Beta 部署模式定版（Edge runtime vs Node.js runtime）；apps/web 之 KYC iframe / WebSocket / Server Actions 設計選擇 |

---

## 3. PoC-A：EdDSA mobile / LIFF SDK 相容性

> 對應 [T-9](../../specs/SPEC-000-index.html)；驗證 SPEC-010 §3.1.2 採 Ed25519 之 JWT 在 LIFF SDK / 主流 mobile JWT lib / browser fetch 端能否正常驗證。

### 3.1 場景

業主指定平台需透過 LINE LIFF mini app 接觸玩家（PRD / SPEC-014 §3.2 / SPEC-017 §6.3）。LIFF mini app 在 LINE app 內以 webview 形式運行，其 JavaScript 環境受限於 LINE webview engine（iOS / Android 各異）。

我們的 JWT access token 採 **EdDSA (Ed25519)** 簽章（SPEC-010 §3.1.2）。LIFF webview 內若需 client-side 驗證 JWT（理論上不應，因 cookie 為 HTTP-only；但若有任何 JavaScript 路徑會接觸到 token），則 webview JS engine 之 `crypto.subtle.verify` API 必須支援 Ed25519。

### 3.2 假設與風險

| 假設                                                                                             | 風險級別                                                                                                              |
| ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| LINE LIFF webview JS engine 對 Ed25519 之 Web Crypto API 支援                                    | **高** — 多數 mobile webview 預設僅支援 RS256 / HS256 / ES256；Ed25519 需要 webview 底層 OpenSSL / BoringSSL 版本支援 |
| 即便 LIFF 不直接驗 JWT（cookie 模式），server-to-server 驗證走 Rust 端 `jsonwebtoken` 0.9+ EdDSA | 低 — Rust crate 已成熟支援                                                                                            |
| 若未來開發 native iOS / Android app，主流 JWT lib（jose / JOSESwift / Auth0 SDK）EdDSA 支援度    | 中 — Apple JOSESwift 自 v3.0 支援；Android `nimbus-jose-jwt` 自 v9.x 支援；舊版本仍多                                 |
| Browser（apps/web）僅走 cookie，不需 client-side 驗證                                            | 低 — 不適用                                                                                                           |

### 3.3 驗證步驟

#### Step 1：LIFF webview 環境調查（不需 LIFF channel）

```bash
# 在 iPhone Safari + Android Chrome 模擬 LIFF webview 之 user-agent，檢查 Web Crypto API
# 任一 sandbox HTML 載入 LIFF SDK，console.log Web Crypto algorithm 支援
```

```javascript
// poc/eddsa/check-support.html
const algo = { name: "Ed25519" };
crypto.subtle
  .generateKey(algo, true, ["sign", "verify"])
  .then(() => console.log("Ed25519 supported"))
  .catch((e) => console.error("Ed25519 NOT supported:", e));
```

**Acceptance**：iOS Safari 17+ / Android Chrome 110+ console 顯示 `Ed25519 supported`。

#### Step 2：LIFF SDK 實機測試（需 LINE Developer Console + LIFF channel）

```bash
# 申請 LIFF channel（業主透過 LINE Developers）
# 建立 sandbox apps/poc-liff/ 之 Next.js mini app
# 部署至臨時 endpoint（Cloudflare Pages preview 或 ngrok）
# 配 LIFF endpoint URL → 實機測試 iPhone / Android LINE app 內開啟
```

LIFF webview 內執行 §Step 1 之 `crypto.subtle.generateKey(Ed25519)`：

**Acceptance**：iPhone LINE app + Android LINE app 兩者皆能執行成功；若任一失敗，記錄具體錯誤訊息。

#### Step 3：Server 端 EdDSA token issue + verify 跨環境

```bash
# 在 services/user-service (sandbox) 用 jsonwebtoken 0.9 issue Ed25519 token
# 同 token 在 LIFF webview / iOS Safari / Android Chrome 三環境呼叫 server endpoint 驗證
# 觀察 cookie 行為、refresh token rotation 行為
```

**Acceptance**：三環境 cookie set + retrieval + server verify 全綠；refresh token rotation（SPEC-010 §3.1.4）跨 LIFF / Browser 一致。

### 3.4 退路選項

若 §3.3 任一 Step 失敗：

| Failure                                                                         | 退路                                                                                                                 | SPEC 影響                                               |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| LIFF webview 不支援 Ed25519 client-side（即便僅 cookie 也踩到 webview JS 限制） | **雙演算法 token issuance**：server 偵測 user-agent，LIFF 路徑簽 RS256，其他路徑簽 EdDSA；cookie 命名加 `_liff` 前綴 | SPEC-010 §3.1.2 補「dual-algo issuer」設計；新 ADR 記錄 |
| iOS Safari < 17 user 比例顯著                                                   | 全平台改 RS256                                                                                                       | SPEC-010 §3.1.2 改演算法                                |
| 僅 Android 部分版本踩坑                                                         | feature-detect + RS256 fallback                                                                                      | SPEC-010 §3.1.2 加 fallback 註記                        |

### 3.5 PoC 執行紀錄（desk research 完成 + live verification 待跑）

> **Artifacts 已就緒**：見 [`eddsa/README.md`](./eddsa/README.md) playbook、[`eddsa/check-support.html`](./eddsa/check-support.html) 環境檢測、[`eddsa/server-token-sample.rs`](./eddsa/server-token-sample.rs) 伺服器端 reference。

```
Status: 🟢 Desk Research Done · 🟡 Live Verification Pending
Desk Research Date: 2026-05-07
Live Executor: TBD (前端 / 全端工程師)
Live Started: TBD
Live Completed: TBD

Desk Research 結論（preliminary，無實機驗證）：
  ✅ iOS Safari 17+ — Web Crypto Ed25519 supported（per WebKit Feature Status）
  ✅ Android Chrome（Chrome 137+ 完整支援；舊版本可能需 polyfill）
  ✅ Cloudflare Workers — Ed25519 supported via subtle.subtle（自 2024 H2）
  ✅ jsonwebtoken (Rust 0.9+) / jose (npm 4+) / Apple CryptoKit / Android BouncyCastle 1.78+ 皆支援
  ⚠️ LINE LIFF webview — webview engine = host Safari/Chrome；舊裝置（iOS 15- / Android 9-）為已知風險點，必須實機驗證

Live Verification 結果（執行後回填）：
  Step 1 (check-support.html，無需 LIFF channel):
    iPhone Safari:        ☐ Pass / ☐ Fail
    Android Chrome:       ☐ Pass / ☐ Fail
  Step 2 (LIFF channel 實機):
    iPhone LINE app:      ☐ Pass / ☐ Fail (note: ___)
    Android LINE app:     ☐ Pass / ☐ Fail (note: ___)
  Step 3 (server token round-trip with eddsa-poc-server):
    Token issuance:       ☐ OK
    Cross-env GET /me:    ☐ OK
    Refresh rotation:     ☐ OK

Final decision: [ ] Proceed with EdDSA  [ ] Dual-algo  [ ] Fallback to RS256
Commit SHA of live verification: ___
```

---

## 4. PoC-B：Cloudflare Pages Edge runtime 限制

> 對應 [T-10](../../specs/SPEC-000-index.html)；驗證 SPEC-014 §3.2.1 列出之五個公開 Beta 前必驗證場景。

### 4.1 場景

`apps/web` 透過 [`@cloudflare/next-on-pages`](https://github.com/cloudflare/next-on-pages) 部署至 Cloudflare Pages。Edge runtime（Cloudflare Workers V8 isolate + Web APIs）與標準 Node.js runtime 行為差異會在以下五個場景踩坑。

### 4.2 五個 PoC 子場景

#### 4.2.1 場景 A：KYC iframe 嵌入（KYC provider Web SDK）

**驗證目標**：KYC provider 之 web SDK iframe 在 Edge runtime 部署之 Next.js page 中能否完整載入 + postMessage 通訊 + Cloudflare Workers `crypto.subtle` API 走 Web Crypto API（不依賴 Node.js `crypto` 模組）。

**步驟**：

1. 建立 sandbox `apps/poc-kyc/app/kyc-test/page.tsx`
2. 載入 KYC provider Web SDK：`<script src="https://provider-sdk.example.invalid/websdk.js"></script>`
3. KYC provider `applicantId` + `accessToken` 經由 Edge runtime API route（`apps/poc-kyc/app/api/kyc-token/route.ts`）由 server 端走 KYC provider REST API 取得
4. iframe 內完成測試證件 OCR + liveness（KYC provider sandbox account 提供測試證件）
5. webhook 送回 server，server 端 HMAC-SHA1 驗證（SPEC-005 §3.3）

**Acceptance**：

- KYC iframe 完整載入無 console error
- HMAC verify 在 Edge runtime 下成功（用 Web Crypto `subtle.importKey` + `subtle.verify`）
- 流程 P95 < 60 秒（SPEC-005 §9.1 happy path）

#### 4.2.2 場景 B：WebSocket fan-out

**驗證目標**：`apps/web` 之 client-side WS 連線（連至 `ws-gateway` 之 axum endpoint，**非** Cloudflare Workers WebSocket）跨 Cloudflare proxy 通過後行為符合 SPEC-003 §10.3。

**步驟**：

1. ws-gateway sandbox 部署於 Linode（或本地 ngrok），暴露 `wss://ws-sandbox.example.com/ws/markets/{id}/book`
2. `apps/poc-ws/app/ws-test/page.tsx` 用 native WebSocket API 連線
3. 模擬 Cloudflare proxy（`apps/poc-ws/wrangler.toml` 配 Cloudflare Pages preview，DNS 指向 sandbox）
4. 測試：subscribe → snapshot → delta → trade → reconnect 流程

**Acceptance**：

- WebSocket connection upgrade 通過 Cloudflare proxy（HTTP 101 Switching Protocols）
- snapshot / delta / trade 訊息順序正確且 `seq` 嚴格遞增
- 中斷重連後 gap detect + re-snapshot 行為正確
- WS message P99 latency < 30ms（SPEC-007 §6.1 / §6.4.3）

#### 4.2.3 場景 C：Server Actions / RSC（Next.js 15 App Router）

**驗證目標**：Server Action 中呼叫後端 gRPC（透過 fetch + tonic-web 或 api-gateway REST proxy）能在 Edge runtime build + run。

**步驟**：

1. `apps/poc-server-action/app/actions.ts`：Server Action 用 `'use server'` 標記
2. Action 內呼叫 `fetch('https://api-sandbox.example.com/api/v1/markets')`
3. RSC 元件呼叫該 Action 並 await 結果

**Acceptance**：

- `pnpm build` 成功（無 Edge runtime 不支援之 Node.js API 警告）
- 部署至 Cloudflare Pages preview 後，Server Action 能成功呼叫並 render

**已知潛在踩坑**：

- 部分 npm package 使用 `Buffer` / `process` / `fs` → 在 Edge runtime build 時失敗
- `tonic-web` 之 server-side proxy 需 axum endpoint 支援 grpc-web，而非純 gRPC

#### 4.2.4 場景 D：fetch 行為差異

**驗證目標**：TanStack Query v5 之 retry / cancel / abort 在 Edge runtime 下行為一致。

**步驟**：

1. `apps/poc-fetch/app/page.tsx` 使用 TanStack Query 配 abortable fetch
2. 模擬 timeout / cancel scenario

**Acceptance**：

- `AbortController` 正確 cancel pending request
- `keepalive: true`（Edge runtime 預設）行為符合預期
- retry policy 不會因 Edge runtime fetch 行為異於 Node.js 而 hang

#### 4.2.5 場景 E：環境變數注入

**驗證目標**：Cloudflare Pages env binding（`wrangler.toml` 之 `[vars]`）與 Next.js `process.env` 在 Edge runtime 行為差異 + secret binding 不誤入 client bundle。

**步驟**：

1. `apps/poc-env/wrangler.toml` 配 `[vars] PUBLIC_FOO = "bar"` + secret binding `[[secrets]] SECRET_KEY`
2. `apps/poc-env/app/page.tsx` 讀 `process.env.PUBLIC_FOO`（client）
3. `apps/poc-env/app/api/secret/route.ts` 讀 `env.SECRET_KEY`（server only）
4. 跑 `pnpm build` 後檢查 `.next` static bundle 是否洩漏 SECRET_KEY

**Acceptance**：

- PUBLIC_FOO 在 client side 可讀
- SECRET_KEY 在 server side 可讀，**不**出現於 client bundle（`grep -r SECRET_KEY .next/static/` 應無命中）
- `wrangler dev` 本地開發時 env 注入路徑與 production 一致

### 4.3 退路選項

若 §4.2 任一場景失敗：

| Failure                                                             | 退路                                                                                            | SPEC 影響                                           |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| 場景 A：KYC iframe 在 Edge runtime 踩 KYC provider 依賴 Node.js API | `apps/web` 改 Cloudflare Pages **Node.js runtime**（性能略降但相容性好）                        | SPEC-014 §3.2 改 deployment mode；ADR 記錄          |
| 場景 B：WS upgrade 在 Cloudflare proxy 失敗                         | ws-gateway endpoint 改用 dedicated subdomain（不過 Cloudflare proxy）                           | SPEC-014 §2 服務拓撲補 WS bypass 路徑；DNS 設計同步 |
| 場景 C：Server Action build 失敗                                    | 改純 client component + REST API 模式；Server Action 拆 `apps/web-bff/` 服務（Node.js Express） | 大改：SPEC-014 §4 新增 web-bff service；新 ADR      |
| 場景 D：fetch 行為異常                                              | 改用 native fetch + 自寫 abort logic，捨棄 TanStack Query                                       | SPEC-014 §3.2 改 data fetching 選擇                 |
| 場景 E：secret 洩漏 client bundle                                   | 強制 secret 走 server-only API route，client 永不接觸                                           | SPEC-010 §3.3 加註；不影響架構                      |

### 4.4 PoC 執行紀錄（desk research 完成 + live verification 待跑）

> **Artifacts 已就緒**：見 [`edge-runtime/README.md`](./edge-runtime/README.md) playbook、[`edge-runtime/scenarios.md`](./edge-runtime/scenarios.md) 5 場景 code snippets。

```
Status: ❌ Live Verification Blocked (Pages adapter output not generated)
Desk Research Date: 2026-05-07
Live Executor: Codex on Windows host (Asia/Taipei)
Live Started: 2026-05-08
Live Completed: 2026-05-08 (blocked before preview deploy)
Cloudflare account/project: ✅ wrangler authenticated as service@dacit.net; created `prophetica-web-poc`
Wrangler CLI: ✅ 4.58.0 (update available: 4.90.0)
Tunnel tooling: ⚠️ `cloudflared` installed; `ngrok` absent; no tunnel URL configured
KYC provider sandbox token: ❌ missing (`KYC_PROVIDER_*` env not present); real KYC provider adapter still throws `not_implemented`
Mobile devices: ❌ not available in Codex host; iOS/Android WebRTC not exercised

Desk Research 結論（preliminary，無實機 deployment）：
  ✅ Scenario A (KYC iframe)        — Likely Pass（KYC provider iframe 走 Web Crypto；Workers V8 已支援）
  ✅ Scenario B (WebSocket fan-out) — Likely Pass（client connect 走 Cloudflare proxy 為 well-known pattern）
  ⚠️ Scenario C (Server Actions)    — Partial Risk（Edge runtime 不支援 fs/net/tls/vm；某些 SaaS SDK 仍會 build fail）
  ✅ Scenario D (fetch behaviour)   — Likely Pass（TanStack Query v5 abstracted；minor edge cases acceptable）
  ✅ Scenario E (env binding)       — Likely Pass（next-on-pages 已 bridge wrangler.toml ↔ process.env 差異）

Live Verification 結果：
  Scenario A (KYC iframe):
    iframe load:        ❌ Blocked before deploy; `.vercel/output/static` was not generated.
    HMAC verify:        ❌ Not run; no `KYC_PROVIDER_SANDBOX_APP_TOKEN` / `KYC_PROVIDER_SANDBOX_SECRET` / webhook tunnel.
    P95 < 60s:          ❌ Not run; no real KYC provider sandbox or mobile device flow.
    offending log:
      - `next-on-pages` on Windows: `SHELLAC COMMAND FAILED! Executing: pnpm --version ... Unexpected error: {"cmd":"pnpm --version","stdout":"","stderr":""}`
      - shim retry: `rtk pnpm --filter @prophetica/web exec next-on-pages` timed out after 240s and left no `.vercel/output/static`.
      - env scan: no `KYC_PROVIDER_*` / `KYC_SERVICE_BASE_URL` values in process env.
  Scenario B (WebSocket fan-out):
    HTTP 101 upgrade:   ❌ Not run; no Pages preview URL and no ws-gateway `wss://` tunnel.
    seq monotonic:      ❌ Not run; no broadcast stream observed.
    reconnect + gap:    ❌ Not run; no deployed client session.
    P99 < 30ms:         ❌ Not run; no remote WS sample.
    offending log:
      - `where.exe ngrok` returned no match.
      - `rg NEXT_PUBLIC_WS_GATEWAY_URL` found only PoC snippet references, no configured endpoint.
  Scenario C (Server Actions):
    pnpm build clean:   ✅ OK for `rtk pnpm --filter @prophetica/web build`; only webpack cache warnings from `next-intl` dynamic import parsing.
    Pages adapter:      ❌ Fail; `next-on-pages` did not produce Pages output on this Windows host.
    Action renders:     ❌ Not run; no preview deployment.
    offending log:
      - `@cloudflare/next-on-pages CLI v.1.13.16`
      - `Warning: ... on a Windows system ... Vercel CLI ... does not work reliably on Windows`
      - `Unexpected error: {"cmd":"pnpm --version","stdout":"","stderr":""}`
  Scenario D (fetch):
    AbortController:    ❌ Not run in Edge preview; blocked by Pages output generation.
    TanStack retry:     ❌ Not run in Edge preview; blocked by Pages output generation.
    offending log:
      - no deployable `.vercel/output/static`; `Test-Path apps/web/.vercel/output/static` returned `False`.
  Scenario E (env binding):
    PUBLIC_FOO client:  ❌ Not run in preview; added `apps/web/wrangler.toml` with non-secret vars only.
    SECRET_KEY server:  ❌ Not run; `KYC_PROVIDER_WEBHOOK_SECRET` not configured via Pages secret in this environment.
    no client leakage:  ✅ Local static bundle check did not find `KYC_PROVIDER_WEBHOOK_SECRET` under `.next/static`; committed config contains only the secret name in a comment, not a value.
    offending log:
      - `Get-ChildItem Env:*KYC_PROVIDER*` returned no entries.
      - `Test-Path apps/web/.vercel/output/static` returned `False`, so Pages env binding could not be exercised.

Final decision:
  [ ] Proceed with Edge runtime (all 5 scenarios pass)
  [ ] Switch to Node.js runtime (any scenario fails build)
  [x] Hybrid / rerun required: app `next build` passes, but Pages adapter generation must run on Linux/CI or a fixed Windows toolchain before Edge-vs-Node runtime can be decided.

Commit SHA of live verification: working tree (not committed)
ADR (if Node.js / Hybrid chosen): pending; do not open until rerun proves runtime failure after Pages output is generated.
```

Follow-up TODO: [`POC-B-LINUX-CI-TODO.md`](./POC-B-LINUX-CI-TODO.md) tracks the Linux / CI rerun steps that can proceed before KYC provider sandbox credentials arrive.

---

## 5. PoC 完成 → 進公開 Beta release review

PoC 完成後，**必須**更新以下 SPEC 章節並隨同 PoC 結果回填本文件：

| PoC   | 必更新章節                                | 形式                                                                                                       |
| ----- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| PoC-A | SPEC-010 §3.1.2 之「Client 相容性 PoC」表 | 將「公開 Beta 前必須完成」改為「✅ 通過 / ⚠️ 部分通過（採 dual-algo） / ❌ 全部 RS256」並補 PoC commit SHA |
| PoC-B | SPEC-014 §3.2.1 之 5 個場景表             | 每個場景補「✅ 通過 / ❌ 失敗 + 退路啟用」標註 + PoC commit SHA                                            |

PoC commit 之版本號 bump 規則：

- SPEC-010 / SPEC-014 之 PoC 結果反映後 bump minor（如 v0.2.1 → v0.2.2）
- ADR-0005 §7 Validation Criteria 對應 checkbox 勾選
- SPEC-000 §5 R-9 / R-10 條目（若 PoC 失敗導致決策變更）標 ✅ Resolved

---

## 6. PoC 失敗之 escalation

任一 PoC failure 達到「啟用退路」程度：

1. 工程師於本文件 §3.5 / §4.4 PoC 紀錄填入失敗證據
2. Tech Lead 評估退路選項對 SPEC 之影響
3. 若退路涉及架構變更（如場景 C 失敗需要 web-bff service）→ 開新 ADR + 對應 SPEC PR
4. 若退路為小範圍配置變更（如場景 E 失敗只需 server-only API route）→ 直接 SPEC patch + commit

PoC 失敗**不**自動阻擋 Codex PR #1 / internal alpha，但對應退路必須**先**落地 SPEC 變更，才可續進公開 Beta release review。

---

## 7. 參考

- [SPEC-010 §3.1.2](../../specs/SPEC-010-security-antifraud.html) — JWT EdDSA 設計與 client 相容性 PoC 表
- [SPEC-014 §3.2.1](../../specs/SPEC-014-tech-architecture.html) — Cloudflare Pages Edge runtime 限制
- [SPEC-017 §11.3](../../specs/SPEC-017-monorepo-bootstrap.html) — PoC launch acceptance hook
- [KYC provider Web SDK Documentation](https://docs.provider.com/docs/websdk-integration)
- [Cloudflare Pages Documentation: next-on-pages](https://developers.cloudflare.com/pages/framework-guides/nextjs/ssr/)
- [LINE LIFF Documentation](https://developers.line.biz/en/docs/liff/overview/)
