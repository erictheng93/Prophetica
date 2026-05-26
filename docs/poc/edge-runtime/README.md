# PoC-B: Cloudflare Pages Edge runtime

> 對應 [SPEC-014 §3.2.1](../../../specs/SPEC-014-tech-architecture.html) + [PRE-FLIGHT-POCS §4](../PRE-FLIGHT-POCS.md)
>
> **狀態**：🟢 Desk Research Done · 🟡 Live Verification Pending（artifacts 已就緒，工程師跑完回填）

---

## 0. TL;DR

- **Desk research 結論（5 個場景）**：A (KYC iframe) / B (WS) / D (fetch) / E (env binding) **預期通過**；C (Server Actions) **需 careful import**（部分 Node.js APIs 不支援）。
- **Live verification 重點**：場景 C 之 KYC provider Web SDK / 撮合相關 SaaS lib 是否在 build 時觸發 Edge runtime 不相容警告；場景 A 之 KYC provider iframe 內部 postMessage 在 Workers proxy 後行為。
- **執行成本**：1 個 Cloudflare Pages preview deployment + 1 個 ws-gateway sandbox endpoint（ngrok）+ 1-2 天工時。
- **退路啟動門檻**：場景 C 失敗 → 改 Cloudflare Pages **Node.js runtime** 或拆 `apps/web-bff/` 後端服務。

---

## 1. Desk Research 結論（2026-05-07）

來源：Cloudflare Pages docs、`@cloudflare/next-on-pages` README、Next.js 15 release notes、KYC provider Web SDK docs。

| 場景                                      | 預期結果        | 關鍵風險                                                                                                                                                                          |
| ----------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A: KYC iframe（KYC provider Web SDK）** | ✅ Likely Pass  | KYC provider iframe 內部依賴 `crypto.subtle` Web Crypto + postMessage；Workers V8 isolate 已支援。風險：iframe 對 host 之 postMessage 在 Cloudflare proxy 後是否 lose origin 資訊 |
| **B: WebSocket fan-out**                  | ✅ Likely Pass  | `apps/web` 不啟用 Cloudflare Workers WebSocket（ws-gateway 為獨立 axum service）；client 連 wss:// 跨 Cloudflare proxy 為 well-known pattern                                      |
| **C: Server Actions / RSC**               | ⚠️ Partial Risk | Edge runtime 不支援 `fs` / `net` / `tls` / `vm` 等 Node.js APIs；多數 npm package 已 conditional-export Edge-friendly variants，但少數 SaaS SDK 仍會 fail at build time           |
| **D: fetch 行為差異**                     | ✅ Likely Pass  | TanStack Query v5 retry / cancel / abort 已 abstracted；Edge runtime fetch keepalive / AbortController minor 差異 acceptable                                                      |
| **E: 環境變數注入**                       | ✅ Likely Pass  | `wrangler.toml` 之 `[vars]` + `[[secrets]]` 走 `env.X` 不走 `process.env`；`@cloudflare/next-on-pages` 已 bridge 此差異                                                           |

**整體判斷**：5 個場景 desk research preliminary 全部「可行」，但場景 C 有顯著機率需要小範圍 retry 或退路啟用。

---

## 2. Live Verification Playbook

### 2.1 預備項目

- [ ] Cloudflare 帳號 + Pages preview deployment 權限（業主帳號）
- [ ] KYC provider sandbox account（業主或測試 token）
- [ ] ws-gateway sandbox endpoint（可用 `ngrok http 8083` 暴露本機 axum service）
- [ ] `wrangler` CLI 安裝 + `pnpm` workspace 環境

### 2.2 5 個場景驗證步驟

詳細測試程式碼見 [`scenarios.md`](./scenarios.md)。每個場景之 acceptance：

#### Scenario A — KYC iframe

1. 將 [`scenarios.md` §A](./scenarios.md#scenario-a) 之 `app/poc-kyc/page.tsx` 拷貝至 `apps/poc-kyc/` 之 sandbox 應用
2. `wrangler pages deploy --project-name=poc-kyc-edge`
3. 桌面 Chrome 打開 deployment URL
4. 完成 KYC provider sandbox 之證件 OCR + liveness（測試 token 提供）
5. 觀察 webhook 接收與 HMAC 驗證是否成功

**Acceptance**：

- iframe 完整載入，無 console error
- Webhook 之 HMAC-SHA1 驗證成功（用 Web Crypto `subtle.importKey` + `subtle.verify`）
- 流程 P95 < 60 秒

#### Scenario B — WebSocket fan-out

1. 在本機跑 `services/ws-gateway` 暴露於 `localhost:8083`
2. `ngrok http 8083` → 取得 HTTPS URL（如 `wss://abc-1234.ngrok-free.app`）
3. 把 [`scenarios.md` §B](./scenarios.md#scenario-b) 之 `app/poc-ws/page.tsx` 部署至 Cloudflare Pages preview
4. 在 deployment URL 跳轉 → client-side WebSocket 連 ngrok endpoint
5. 模擬 subscribe → snapshot → delta → trade → reconnect

**Acceptance**：

- WS upgrade 過 Cloudflare proxy（HTTP 101 Switching Protocols）
- snapshot / delta / trade 訊息順序正確且 `seq` 嚴格遞增
- 中斷重連後 gap detect + re-snapshot 正確
- WS message P99 latency < 30ms

#### Scenario C — Server Actions

1. 部署 [`scenarios.md` §C](./scenarios.md#scenario-c) 之 `app/poc-actions/`
2. 跑 `pnpm build`，記錄是否有 Edge runtime 不相容 warning
3. Server Action 內呼叫 `fetch('https://api-sandbox.example.com/api/v1/markets')` mock endpoint

**Acceptance**：

- `pnpm build` 無 Node.js API 不相容 warning
- Server Action 在 deployment 上成功 render

**已知踩坑**：若任一 transitive npm package 用 `Buffer` / `process.cwd()` / `fs` → build 時 fail。退路：改 client component + REST API 或 拆 `apps/web-bff/`。

#### Scenario D — fetch 行為

1. 部署 [`scenarios.md` §D](./scenarios.md#scenario-d)
2. 模擬 timeout / cancel scenario，檢查 `AbortController` 行為

**Acceptance**：

- `AbortController` 正確 cancel pending request
- TanStack Query retry policy 不會因 Edge runtime fetch 行為異於 Node.js 而 hang

#### Scenario E — 環境變數注入

1. 設定 [`scenarios.md` §E](./scenarios.md#scenario-e) 之 `wrangler.toml`：`[vars] PUBLIC_FOO = "bar"` + secret binding `[[secrets]] SECRET_KEY`
2. 部署後檢查：
   - client side `process.env.PUBLIC_FOO` 可讀
   - server side API route 之 `env.SECRET_KEY` 可讀
   - `grep -r SECRET_KEY .next/static/` 無命中（不洩漏 client bundle）

**Acceptance**：所有 3 個 check 通過。

### 2.3 退路選項（依場景失敗組合）

| 失敗組合                | 退路                                                                                                                    | SPEC 影響                                                    |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| **僅 C 失敗**           | 改 Cloudflare Pages **Node.js runtime**（性能略降但相容性好）                                                           | SPEC-014 §3.2 改 deployment mode；新 ADR 紀錄                |
| **C + 其他多項失敗**    | 拆 `apps/web-bff/` 後端服務（Node.js Express）；`apps/web` 僅 SSR + 純前端互動；Server Action / KYC provider 整合落 BFF | SPEC-014 §4 新增 web-bff service；新 ADR；CI / Helm 同步擴展 |
| **僅 B WebSocket 失敗** | ws-gateway 改用 dedicated subdomain 不過 Cloudflare proxy                                                               | SPEC-014 §2 服務拓撲補 WS bypass 路徑；DNS 設計同步          |
| **僅 E secret 洩漏**    | 強制 secret 走 server-only API route                                                                                    | SPEC-010 §3.3 加註                                           |

---

## 3. Acceptance Sign-off

PoC-B 執行完畢 → 回填 [PRE-FLIGHT-POCS §4.4](../PRE-FLIGHT-POCS.md)：

```
Status: ✅ Pass / ❌ 退路啟用: <選項>
Executor: <工程師>
Started: <YYYY-MM-DD>
Completed: <YYYY-MM-DD>
場景 A (KYC iframe): <result + commit SHA>
場景 B (WebSocket): <result>
場景 C (Server Actions): <result, 標明 build warnings>
場景 D (fetch): <result>
場景 E (env binding): <result>
Final decision: [ ] Edge runtime  [ ] Node.js runtime  [ ] Hybrid (web Edge + web-bff Node.js)
```

若選 Node.js runtime 或 Hybrid → 開 SPEC-014 patch PR + 新 ADR。
