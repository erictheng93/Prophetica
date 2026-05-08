# SPEC-005a — SumSub 前端整合契約

> 優先級：**P1**（與 SPEC-005 相同）
> 版本：v0.1.1 · 最後更新：2026-05-08
> 依賴：SPEC-005, SPEC-007, SPEC-008, SPEC-014
> 被依賴：apps/web KYC 流程、未來其他 KYC provider 切換
> 實作狀態：⚠️ **Contract 規格完成 / 實作待 PoC-B 通過後啟動**

---

## 0. Document Purpose

本 SPEC 定義 prophetica 前端 (`apps/web`) 與 KYC SaaS provider SDK 之間的整合契約，目的有三：

1. **解構 SPEC-005 §0.3 第一列阻擋**：在 SumSub 合約 / sandbox 帳號到位前，先把「placeholder → 真實 SDK」這個 swap 可以走的路徑寫死，讓真 SDK 拿到時是純 adapter 替換工作。
2. **provider-agnostic**：契約以抽象的 `KycIdentityProvider` interface 表達，未來若從 SumSub 切到 Onfido / Jumio，只動 adapter，不動 page 組件、不動 ts-sdk。
3. **修正 SPEC-005 review B2 結構性問題**：明確流程順序，避免「先建 pending record → 再不掛 iframe → record 卡死」的反 pattern。

> 本 SPEC 不重複 SPEC-005 §2 的 Tier 規則、§3.3 webhook 後端契約、§4 schema、§5 PII 政策。請以那些章節為合規 source of truth。

---

## 1. Contract Scope（範圍邊界）

### 1.1 在 scope 內

- Tier 2 / Tier 3 升級頁面（`apps/web/app/kyc/tier-2`, `apps/web/app/kyc/tier-3`）與 KYC provider SDK 之間的 client-side 契約
- `@prophetica/ts-sdk` 對 kyc-service 後端 API 的呼叫順序
- SDK iframe 的 mount / unmount 生命週期
- SDK 事件 → UI 狀態轉換 → 指標 / log 路由
- 前端錯誤處理與重試
- CSP / Edge runtime 相容性需求

### 1.2 不在 scope 內

- Tier 1 自驗（純後端規則，無 SDK）— 見 SPEC-005 §2.4
- Webhook 後端處理 — 見 SPEC-005 §3.3
- 證件影像儲存 / 加密 — 見 SPEC-005 §5
- AML 規則引擎 — 見 SPEC-005 §7

---

## 2. Core Abstraction：`KycIdentityProvider`

定義單一抽象，前端組件**只**依賴此 interface，不直接 import SumSub / Onfido SDK：

```ts
// packages/ts-sdk/src/kyc-provider/types.ts

export interface KycSessionStartParams {
  /** sdk_token from POST /api/v1/kyc/users/{id}/upgrades */
  sdkToken: string;
  /** target tier (TIER_2 or TIER_3) */
  targetTier: "TIER_2" | "TIER_3";
  /** locale code: zh-TW, ja-JP, ko-KR, vi-VN, ms-MY, en */
  locale: string;
  /** user's country code from Tier 1 (ISO 3166-1 alpha-2) */
  country: string;
}

export interface KycSessionHandle {
  /** Unmounts the iframe and releases all resources. Idempotent. */
  unmount(): void;
  /** Subscribe to lifecycle events. Returns unsubscribe fn. */
  subscribe(listener: KycEventListener): () => void;
}

export type KycEventListener = (event: KycSessionEvent) => void;

export type KycSessionEvent =
  | { kind: "ready" }
  | { kind: "step_changed"; step: KycStep }
  | { kind: "submitted" }                     // user uploaded all docs, awaiting backend webhook
  | { kind: "completed"; outcome: KycInlineOutcome }   // SDK self-determined outcome (rare for SumSub)
  | { kind: "abandoned" }                     // user closed iframe
  | { kind: "error"; code: KycErrorCode; recoverable: boolean };

export type KycStep = "consent" | "document" | "selfie" | "liveness" | "address" | "review";

export type KycInlineOutcome = "approved" | "rejected" | "pending_review";

export type KycErrorCode =
  | "sdk_load_failed"      // network / CSP / script tag failure
  | "sdk_token_expired"    // sdk_token expiry from backend
  | "browser_unsupported"  // missing camera / WebRTC / Permissions API
  | "permission_denied"    // user denied camera / microphone
  | "network_lost"         // sustained connectivity loss during session
  | "unknown";

export interface KycIdentityProvider {
  readonly name: "sumsub" | "iproov" | "onfido" | "jumio" | "mock";
  mount(target: HTMLElement, params: KycSessionStartParams): Promise<KycSessionHandle>;
}

export interface KycAdapterStage {
  readonly name: KycProviderName;
  readonly required: boolean;
  readonly continueAfterSubmit: boolean;
}

export interface KycAdapterChain {
  readonly stages: KycAdapterStage[];
  readonly mountStrategy: "sequential";
}
```

**關鍵紀律**：
- `KycInlineOutcome` 不等於 SPEC-005 §4 的 `KycStatus`。SDK inline 結果只是 best-effort 提示（多數情況 SumSub 會回 `pending_review`，最終結果由 webhook 決定）。
- `subscribe` 永遠回傳 unsubscribe fn，避免 memory leak。
- `mount` 失敗（reject）時不應 throw `error` event，而是 reject promise。組件應 catch 並渲染 fail-soft 狀態。

---

## 3. Provider Adapter Layout

```
packages/ts-sdk/src/kyc-provider/
├── index.ts                    # exports types + default provider resolver
├── types.ts                    # KycIdentityProvider, KycSessionEvent, etc.
├── sumsub-provider.ts          # SumSub Web SDK adapter (real)
├── mock-provider.ts            # In-memory adapter for tests / dev
└── select-provider.ts          # Resolves provider from env / sdk_token prefix
```

### 3.1 Provider 選擇規則

```ts
// select-provider.ts (pseudocode)
export function selectKycProvider(opts: {
  sdkToken: string;
  override?: KycIdentityProvider;
}): KycAdapterChain {
  if (opts.override) return singleStageChain(opts.override.name); // for tests
  if (opts.sdkToken.startsWith("sumsub_sandbox_")) return sumsubChain(opts.sdkToken);
  if (opts.sdkToken.startsWith("sumsub_")) return sumsubChain(opts.sdkToken);
  if (opts.sdkToken.startsWith("mock_")) return singleStageChain("mock");
  throw new KycSdkError("unknown_sdk_token_prefix");
}
```

> **與 kyc-service 對齊**：SPEC-005 §3 中 kyc-service 目前對 Tier 2 / Tier 3 之 `provider = "sumsub"` 並產生 `sdk_token = "sumsub_sandbox_{user}_{tier}_{id}"`。本契約的 `mockSumsubProvider` 認 `sumsub_sandbox_` 前綴；若後端切真實 SumSub，需要把 token 前綴改為 `sumsub_` 純 production prefix（kyc-service 改 1 行即可），此即 placeholder → 真 SDK 的核心 swap point。

### 3.2 SumSub Adapter 內部需求

`sumsub-provider.ts` 的 mount 流程必須：

1. 動態載入 `https://static.sumsub.com/idensic/static/sns-websdk-builder.js`（**不**靜態 import，避免 SSR build 失敗）
2. 用 `snsWebSdk.init(sdkToken, () => fetchNewToken())` 建立 instance
3. 註冊 `idCheck.onReady` / `idCheck.onError` / `idCheck.onApplicantStatusChanged` / `idCheck.onApplicantSubmitted` 等 SumSub 原生事件
4. 把原生事件 map 到 `KycSessionEvent`（見 §4）
5. `launch(target.id)` 掛載到 `target` DOM
6. `mount` 回傳 `KycSessionHandle` 包裹 `instance.destroy()` 為 `unmount`

**禁止**：在 adapter 之外的任何地方 import `@sumsub/websdk`、寫 `snsWebSdk` 字面量、或對 SumSub 原生事件做 mapping。違反即破壞 provider-agnostic 不變式。

### 3.3 Mock Adapter 行為（dev / test）

`mock-provider.ts` 必須：

1. 在 `mount` 後立即 emit `{ kind: "ready" }`
2. 在 target 內渲染**可見但顯眼地標示 MOCK** 的 stub UI（用 `<div data-testid="kyc-mock-stub">`）
3. 提供 `globalThis.__mockKycComplete(outcome)` 全域 hook，供 e2e test 觸發 `submitted` / `completed` 事件
4. 預設 5 秒後自動 emit `{ kind: "submitted" }` 模擬使用者完成上傳
5. 永遠不打真實網路請求

### 3.4 Adapter Chain Contract

Tier 2 remains a single-stage chain. Tier 3 is a sequential chain: SumSub first, iProov second. The contract is a manifest, not an implementation of the real iProov adapter.

```ts
export interface KycAdapterStage {
  readonly name: KycProviderName;
  readonly required: boolean;
  readonly continueAfterSubmit: boolean;
}

export interface KycAdapterChain {
  readonly stages: KycAdapterStage[];
  readonly mountStrategy: "sequential"; // "parallel" reserved for future
}
```

Canonical manifests:

```ts
const tier2Chain: KycAdapterChain = {
  stages: [{ name: "sumsub", required: true, continueAfterSubmit: false }],
  mountStrategy: "sequential",
};

const tier3Chain: KycAdapterChain = {
  stages: [
    { name: "sumsub", required: true, continueAfterSubmit: true },
    { name: "iproov", required: true, continueAfterSubmit: false },
  ],
  mountStrategy: "sequential",
};
```

Semantics:

- `continueAfterSubmit: true`: after the SDK emits `submitted`, dispatch `stage_advance`, mount stage N+1, and do not enter `awaiting_webhook`.
- `continueAfterSubmit: false`: after the SDK emits `submitted`, enter `awaiting_webhook` and wait for webhook-backed terminal status.
- Any non-recoverable error from a `required: true` stage aborts the chain.
- Stages share `providerApplicantId`; each stage receives its own backend-issued `sdkToken`.
- `iproov-provider.ts` and `iproov-mock-provider.ts` are adapter slots only in this slice and must throw `KycSdkError("not_implemented")` until PR #2 wires sandbox credentials and real mount behavior.

### 3.5 Tier 3 Routing and Backend Response

`kyc-service start_upgrade` for `TIER_3` must return the stage manifest with the initial stage token:

```json
{
  "sdkToken": "...",
  "providerApplicantId": "...",
  "chain": {
    "stages": [
      { "name": "sumsub", "required": true, "continueAfterSubmit": true },
      { "name": "iproov", "required": true, "continueAfterSubmit": false }
    ],
    "mountStrategy": "sequential"
  }
}
```

The frontend may derive the same manifest from `sdkToken` for backward compatibility, but backend-supplied `chain` is authoritative once present. Backend implementation is deferred to PR #2; this SPEC only fixes the response field and frontend/SDK contract.

---

## 4. Event Mapping（SumSub native → KycSessionEvent）

| SumSub event | `KycSessionEvent` | 後續 UI 行為 |
|--------------|-------------------|--------------|
| `idCheck.onReady` | `{ kind: "ready" }` | 隱藏 loading skeleton |
| `idCheck.onStepInitiated` | `{ kind: "step_changed", step: <map> }` | 更新進度條 |
| `idCheck.onApplicantSubmitted` | `{ kind: "submitted" }` | 顯示「審核中，可離開頁面」（SPEC-005 §8.1） |
| `idCheck.onApplicantStatusChanged` | (無 1:1 對應) | 不主動 emit；最終狀態以後端 webhook 為準 |
| `idCheck.onApplicantInlineCompleted` | `{ kind: "completed", outcome: "pending_review" }` | 同 `submitted` |
| `idCheck.onError` | `{ kind: "error", ... }` | 渲染 `KycErrorCard` |
| `Modal close` (user X) | `{ kind: "abandoned" }` | 留在當頁，提示 retry |

### 4.1 Step naming

SumSub step name 不直接傳給 UI（避免 i18n 漏鎖），由 adapter 翻譯為 `KycStep` enum：

```
SumSub: "IDENTITY"           → KycStep: "document"
SumSub: "IDENTITY2"          → KycStep: "document"  (multi-doc workflow)
SumSub: "SELFIE"             → KycStep: "selfie"
SumSub: "PROOF_OF_ADDRESS"   → KycStep: "address"
SumSub: "QUESTIONNAIRE"      → KycStep: "consent"
其他                         → KycStep: "review"
```

### 4.2 Error code mapping

| SumSub error | `KycErrorCode` | recoverable |
|--------------|----------------|-------------|
| `unauthorized` / token expiry | `sdk_token_expired` | true（refresh token 重試） |
| `network_error` | `network_lost` | true |
| Camera / Microphone permission rejected | `permission_denied` | true（提示使用者開啟權限） |
| Browser missing required APIs | `browser_unsupported` | false（建議切桌面瀏覽器） |
| Other | `unknown` | false |

---

## 5. Tier 2 Page Flow（修正 review B2）

下述為 `apps/web/app/kyc/tier-2/page.tsx` 必須實作的順序。**注意：先呼叫 `startKycUpgrade` 取得 `sdk_token` 才掛 iframe**，與目前 placeholder 結構不同。

### 5.1 State machine

```
Idle
  ↓ user submits consent + (optional) income bucket
Initializing                     // calling POST /upgrades
  ↓ sdk_token received
Mounting                         // provider.mount() in flight
  ↓ provider emits "ready"
InProgress                       // SDK iframe interactive
  ↓ provider emits "submitted"
AwaitingWebhook                  // poll GET /status
  ↓ status="passed"
Done
  ↓ status="rejected" | "frozen" | "expired"
Failed
  ↓ provider emits "abandoned"
Idle (with retry CTA)
  ↓ provider emits "error"
Error (recoverable=true → retry button) | Error (recoverable=false → support CTA)
```

### 5.2 React component contract

```tsx
// apps/web/app/kyc/tier-2/page.tsx (target shape)

"use client";  // The SDK requires browser context; cannot stay as Server Component.

export default function Tier2Page() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<KycErrorCode | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleStart = async (formData: FormData) => {
    setPhase("initializing");
    try {
      const upgrade = await startKycUpgradeAction({
        targetTier: "TIER_2",
        declaredMonthlyIncome: incomeBucketToMinorUnits(formData.get("incomeBucket")),
        retryAttempt,
      });
      const provider = selectKycProvider({ sdkToken: upgrade.sdkToken });
      const handle = await provider.mount(containerRef.current!, {
        sdkToken: upgrade.sdkToken,
        targetTier: "TIER_2",
        locale: nextIntlLocale,
        country: tier1Country,
      });
      handle.subscribe((event) => routeKycEvent(event, setPhase, setError));
    } catch (e) {
      reportKycError(e);
      setPhase("error");
    }
  };

  return /* ... */;
}
```

### 5.2.1 Server action trust boundary（硬性契約）

KYC server action 的 client input **不得包含使用者身份**，包含但不限於 `userId`、`externalUserId`、`applicantId`、或任何可推導 user subject 的欄位。唯一可信身份來源是 server-side session / auth context。

`startKycUpgradeAction` 的 public input shape 只能包含：

```ts
{
  targetTier: "TIER_2" | "TIER_3";
  declaredMonthlyIncome?: number | null;
  retryAttempt?: number;
}
```

Server action 必須：

1. 從 server-side session 解析 authenticated `userId`。
2. 使用該 server-derived `userId` 建立 kyc-service client auth context（例如 `x-user-id`）。
3. 在 server side 重建 idempotency key（見 §5.3）；不得接受 client 傳入的 key。
4. 不把 client 傳入的 user-like 欄位 forward 到 kyc-service。

kyc-service 仍必須在 service boundary enforce `Path(user_id)` 與 authenticated subject 一致；web action 的型別限制只是第一層防線。

### 5.3 Idempotency key 設計

**問題**：使用者連點兩次 → ts-sdk 自動 generate UUIDv7（client.ts:91）→ 後端建兩個 pending record。

**修正**：server action 必須用 server-derived `userId` 產生**確定性 key**：

```
idempotency_key = `kyc:${userId}:${targetTier}:${currentTier1RecordId}`
```

意義：「user X 從目前 Tier 1 record 升級到 Tier 2」是同一語意操作，多次點擊應 dedupe 到同一 key。後端 `start_upgrade` 已實作以 idempotency_key 為主鍵的 dedupe（kyc.rs:461-472），會回 `replayed: true` 並回原 sdk_token。client 不可自行計算或傳入此 key，避免偽造其他使用者 key 造成 collision / replay。

> 若使用者在 SumSub 那端中斷後想重來，UI 應提供「重新驗證」按鈕，內部 bump 一個 `attempt` counter 加進 key 後綴：`kyc:${userId}:TIER_2:${tier1Id}:retry${n}`。SPEC-005 §8.2「24h 5 次」由後端執行（待實作，review M10）。

---

## 6. Polling vs Realtime（webhook 抵達後通知前端）

**SPEC-005 §3.3** webhook 寫入 backend record 後，前端需要知道。兩種策略：

### 6.1 Phase 1：Polling（MVP）

- 使用者進入 `AwaitingWebhook` 後，每 3s `fetchKycOverview()`
- 上限 5 分鐘（SPEC-005 §8.1「2-15 分鐘」之保守值）
- 5 分鐘未變更 → 顯示「審核時間較長，已寄信通知，您可離開頁面」

### 6.2 Phase 2：WebSocket（與 SPEC-008 §7 對齊）

- 訂閱 `kyc:{userId}` channel
- Backend webhook handler 處理完即 publish 一條 `{ type: "kyc.status_changed", status: "passed" }`
- 前端 `connection-status-banner.tsx` 已建立的 reconnecting WS 客戶端可重用

> Phase 2 阻擋於 SPEC-008 §7 的 ws-gateway 對外開放規劃。Phase 1 polling 必須先實作，避免 polling 成為永久 fallback。

---

## 7. PII 與 Log 紀律（呼應 SPEC-007 §4.3）

### 7.1 絕對禁止寫入 client-side log

- 整個 `KycSessionEvent` payload — 即使 `step` 字面量也避免（雖然不算 PII，但減少 surface area）
- SumSub 原生 event 物件
- `sdk_token`（雖非 PII 但可被用於 session hijack）
- `applicant_id`（與後端 webhook 配對的 internal ID）
- 證件圖片任何元素

### 7.2 允許的 log

```ts
logger.info("kyc.session.event", {
  event_kind: event.kind,
  recoverable: event.kind === "error" ? event.recoverable : undefined,
  // NO step name, NO error code body, NO outcome detail
});
```

### 7.3 Telemetry metric 命名（呼應 SPEC-007 §3）

| Metric | Type | Labels |
|--------|------|--------|
| `proph_kyc_session_started_total` | Counter | `tier`, `provider` |
| `proph_kyc_session_completed_total` | Counter | `tier`, `provider`, `outcome` |
| `proph_kyc_session_abandoned_total` | Counter | `tier`, `provider`, `step_at_abandon` |
| `proph_kyc_session_error_total` | Counter | `tier`, `provider`, `code`, `recoverable` |
| `proph_kyc_webhook_wait_seconds` | Histogram | `tier`, `outcome` |

`step_at_abandon` 可用 `KycStep` enum（document/selfie/...），不是 SumSub 原始字面量。

Implementation:
- Metric contract and enum-bounded validation: `apps/web/lib/telemetry/kyc-metrics.ts`
- OpenMetrics renderer and POST/GET handler logic: `apps/web/lib/telemetry/kyc-metrics-handler.ts`
- Scrape endpoint: `apps/web/app/api/metrics/route.ts`
- Prometheus scrape job: `infra/prometheus/prometheus.yml`

Histogram buckets for `proph_kyc_webhook_wait_seconds` are `[1, 5, 15, 30, 60, 120, 300]` seconds.

Production deployment note: the current implementation uses in-process memory and is acceptable for local/dev scrape validation. Cloudflare Pages isolates do not provide durable cross-request or cross-instance metric storage, so production deployment must export these events to a durable sink such as Cloudflare Workers Analytics Engine, OTLP, or a push gateway.

---

## 8. CSP（Content Security Policy）需求

SumSub Web SDK 需要載入腳本與打 API。`apps/web` 之 `next.config.ts` 必須加：

```
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'unsafe-inline' https://static.sumsub.com;
  connect-src 'self' https://api.sumsub.com https://*.sumsub.com wss://*.sumsub.com;
  frame-src 'self' https://*.sumsub.com;
  img-src 'self' data: blob: https://*.sumsub.com;
  media-src 'self' blob:;
  worker-src 'self' blob:;
```

Mock provider **不**需要 CSP 開放，所有 dev / test 必須在 `script-src 'self'` 下運作。

---

## 9. Edge Runtime 相容性（呼應 PoC-B）

> docs/poc/PRE-FLIGHT-POCS.md §4.2.1 將驗證 SumSub iframe 在 Cloudflare Pages Edge runtime 是否可用。

### 9.1 已知潛在問題

- `sumsub-provider.ts` 動態 `import("https://...")` 在 Edge runtime 可能 build fail
- SumSub SDK 內部使用之 `XMLHttpRequest` / `Document.createElement('script')` 應在 client-side 才執行（`"use client"` 已可隔離）
- Tier 2 page 必須是 `"use client"` 而非 React Server Component

### 9.2 退路

若 PoC-B §4.2.1 失敗：
- 將 `/kyc/tier-2` 與 `/kyc/tier-3` route 標記為 `runtime: "nodejs"`，不走 Edge runtime
- SPEC-014 §3.2.1 已預留此選項

---

## 10. Acceptance Criteria

- [ ] `packages/ts-sdk/src/kyc-provider/` 目錄與檔案結構建立
- [ ] `KycIdentityProvider` interface + `KycSessionEvent` union 定義通過 `tsc --noEmit`
- [ ] `mockProvider` 與 `mockSumsubProvider` 實作 + 對應 `node:test` 通過
- [ ] `apps/web/app/kyc/tier-2/page.tsx` 重構為 `"use client"` + 使用 `selectKycProvider`
- [ ] State machine 全部 6 phases 都有對應 i18n copy（next-intl `kyc.tier2.phase.*`）
- [ ] 確定性 idempotency key 實作通過 e2e：點兩次 submit 後端只建一筆 pending record
- [ ] Polling 5 分鐘上限通過 e2e：mock webhook 在 t=2.5min 抵達後 UI 自動進 `Done`
- [ ] CSP 字串加入 `apps/web/next.config.ts` 並通過 build
- [ ] Mock provider 不觸發任何外部網路請求（vitest network mocking 驗證）
- [x] Telemetry metric 5 條全部 emit 並通過 prometheus integration test
- [ ] 無 PII / `applicant_id` / `sdk_token` 寫入 console — `pnpm --filter @prophetica/web lint --rule no-console` 通過
- [ ] 真實 SumSub adapter 待 SumSub sandbox 帳號到位後，**只**改 `select-provider.ts` 一處 prefix 判定 + `sumsub-provider.ts` 實作即可

---

## 11. Open Questions

1. SumSub `applicantInlineCompleted` 的 outcome 是否可信？是否任何情況都應仍 `AwaitingWebhook`？建議查 SumSub 文件並補附錄。
2. ✅ resolved (2026-05-08): Tier 3 uses the sequential SumSub + iProov adapter chain defined in §3.4. `selectKycProvider` returns `KycAdapterChain`; the old single-provider path is the `chain.stages.length === 1` case. Real iProov adapter implementation remains PR #2.
3. ✅ resolved (2026-05-08): `step_at_abandon` is enum-bounded to `KycStep | "unknown"` in `apps/web/lib/telemetry/kyc-metrics.ts`; free text is rejected by validation before entering the registry.
4. CSP 是否影響其他 SaaS（Sift / Sardine 行為指紋 SDK 將在 SPEC-010 落地）？需協調 single global CSP 而非 per-route。
5. WebSocket Phase 2 切換的 contract（事件名 / topic 命名）需與 SPEC-008 §7 / ws-gateway 對齊。

---

## 12. 變更紀錄

| 日期 | 版本 | 作者 | 變更 |
|------|------|------|------|
| 2026-05-08 | v0.1.1 | Tech Lead | 補 §5.2.1 server action trust boundary：client input 不得包含 user identity；server-side session 為唯一 source of truth；idempotency key 必須由 server action 以 server-derived `userId` 重建，kyc-service 仍需 enforce subject/path 一致。 |
| 2026-05-08 | v0.1.0 | Tech Lead | 初版。回應 SPEC-005 review B2（流程結構錯置）+ SPEC-005 §0.3 第一列 placeholder 阻擋。定義 `KycIdentityProvider` 抽象、SumSub adapter 邊界、Tier 2 page state machine、idempotency key 確定性、polling fallback、PII log 紀律、CSP / Edge runtime 需求。 |
