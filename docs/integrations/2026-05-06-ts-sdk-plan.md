# ts-sdk Integration Plan — Backend Merge Audit

> Date: 2026-05-06
> Owner: Tech Lead
> Status: Audit + plan; not yet executed

---

## 1. Backend state (Codex)

### 1.1 Shipped — actually wireable

**`services/wallet-ledger-service`** (12 commits, by far the deepest service)

HTTP API surface (`services/wallet-ledger-service/src/api.rs`, axum):

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/internal/ledger/transactions` | post a multi-entry double-entry transaction with idempotency_key |
| `GET`  | `/internal/ledger/wallets/:user_id/balance` | wallet balance (available / locked / frozen, minor units) |
| `GET`  | `/internal/ledger/entries?account_code=...&limit=...` | journal entries for a single account |

Behaviour proven by 12 commits:
- idempotency persistence (`6062548`)
- reconciliation runs + findings table (`d11c555` / `179ed10` / `0782466`)
- gateway settlement reconciliation (`179ed10`)
- liability bound enforcement + serialised balance writes (`9324ffd` / `f159b15`)
- safety-check hardening (`e253ee0`)

Read APIs return JSON; `txn_id` is `i64`; `*_minor_units` is `i64` (per SPEC-002 §3.2 internal precision = TWD × 100). Response shapes exposed in `api.rs` are already `serde::Serialize` structs with stable field names.

### 1.2 Stubbed — directory exists, business code does not

```
services/{api-gateway, ws-gateway, kyc-service, market-service,
matching-engine, position-service, settlement-worker,
risk-antifraud-service, referral-service, notification-service,
user-service}/src/
  main.rs   ≈ 278 bytes
  lib.rs    ≈  82 bytes
```

These are SPEC-017 §6 hello-world stubs. No business endpoints, no database integration. Treating them as "exists" today would be misleading.

### 1.3 Proto state

`packages/proto/v1/` — 7 .proto files with service definitions:

| .proto | Service | Notable RPC |
|---|---|---|
| `engine.proto` | `EngineService` | PlaceOrder / CancelOrder / GetBookSnapshot |
| `ledger.proto` | (TBD) | implied by api.rs but the .proto is the gRPC contract, not the HTTP one |
| `market.proto` | (TBD) | covered by SPEC-015 / SPEC-003 |
| `kyc.proto` | (TBD) | SPEC-005 |
| `payment.proto` | (TBD) | SPEC-016 |
| `errors.proto` | — | shared `ErrorEnvelope` |
| `common.proto` | — | shared types |

`packages/proto/gen/ts/proph/v1/` — TypeScript stubs exist (buf-managed). Whether they're committed and current vs `.proto` source needs CI verification (per SPEC-017 §8.1 there's a `git diff --exit-code` gate planned).

### 1.4 ts-sdk state

`packages/ts-sdk/` is a **bootstrap stub**:

```ts
export const propheticaSdkBootstrap = { status: "bootstrapping" } as const;
```

No client, no fetch wrapper, no proto re-export. Empty workspace package.

---

## 2. What apps/web depends on (today, mocked)

`apps/web/lib/mock-markets.ts` documents the expected RPC contract:

```
MarketService.GetMarket / GetOrderBook / GetRecentTrades / GetPriceHistory
```

`apps/web/lib/mock-portfolio.ts` depends on:

```
PositionService — list positions, get cost basis
WalletLedger    — balance + recent transactions for points display
```

KYC / RG / topup pages have no live data dependencies (forms are disabled). Their integration is **submission**, not query.

---

## 3. Wireable today vs blocked

### 3.1 Wireable — `wallet-ledger-service` HTTP

The only non-stub service. apps/web uses points balance + recent ledger entries in two places:

- `/portfolio` summary cards (totalCost / currentValue / etc.) — **partially** wireable: balance is queryable, but P/L requires position-service which is a stub.
- `/topup` BalanceBlock — **fully** wireable: needs balance + monthly deposit aggregation. `list_entries` over the player's deposit account would do it.

Recommended first integration: **topup balance display** (smallest blast radius, single endpoint).

### 3.2 Not wireable — everything else needs a stubbed service to ship code

- Markets (list / detail / order book / recent trades / chart series): blocked on `market-service` + `matching-engine`
- Portfolio positions: blocked on `position-service`
- KYC tier display + submission: blocked on `kyc-service`
- Trade panel order submission: blocked on `matching-engine` + (RG / KYC pre-checks via api-gateway middleware)
- Topup payment flow: blocked on payment provider integration (out of repo scope until SPEC-016 §3.2 channel verification clears)

---

## 4. ts-sdk plan (phased)

### Phase 1 — Foundation (this week)

Goal: usable client for wallet-ledger HTTP, with conventions other services will inherit.

```
packages/ts-sdk/
  src/
    index.ts                 # re-exports
    runtime/
      fetch.ts               # typed fetch wrapper, ErrorEnvelope decoding
      idempotency.ts         # uuid-v7 generator (post txn requires idempotency_key)
      auth.ts                # Bearer / cookie attach, refresh hook
    wallet/
      client.ts              # postTransaction / getBalance / listEntries
      types.ts               # PostTransactionRequest etc, mirrored from api.rs serde
  package.json               # add: zod, uuid (peer), declare workspace dep on @prophetica/proto
```

Conventions:
- **HTTP first, gRPC later.** wallet-ledger ships HTTP only today; gRPC for other services arrives via tonic in stubs. ts-sdk runtime supports both via base URL + transport adapter.
- **Zod schemas at the boundary.** Each response decoded through a zod schema generated from / aligned with serde structs. Drift caught at runtime, not on first user click.
- **No global state.** Client constructed per request (server components) or per session (client components). Auth header injection through builder pattern.

Acceptance:
- Typed `postTransaction(...)` round-trips against a local wallet-ledger
- `getBalance(userId)` returns `{ availableMinorUnits, lockedMinorUnits, frozenMinorUnits }` as `bigint` (i64 → bigint, not number)

### Phase 2 — First UI integration

Replace `/topup` BalanceBlock mock with real call:

```ts
import { walletClient } from "@prophetica/ts-sdk/wallet";

const balance = await walletClient.getBalance(userId);
```

Concrete migration target: `apps/web/app/topup/page.tsx` BalanceBlock. Mock `mockState` becomes a fallback for unauthenticated demo state; authenticated state pulls live.

This phase exercises the auth layer end-to-end (api-gateway routes /api/wallet/* to wallet-ledger after middleware enforces JWT + RG + KYC checks). **Blocked on api-gateway shipping the routing — not on wallet-ledger itself.**

### Phase 3 — Service-by-service

As each service ships beyond stub, add a client:

```
src/
  market/    # GetMarket, GetOrderBook, GetRecentTrades, GetPriceHistory
  positions/
  kyc/
  notifications/
```

Each client follows the wallet pattern (runtime, types, schema). Replace mock-markets.ts / mock-portfolio.ts adapter function bodies one at a time; signatures already match per the comment block in `mock-markets.ts`.

### Phase 4 — gRPC / streaming

Once `matching-engine` ships, add gRPC-Web (or HTTP/2 via api-gateway) for:
- order book live deltas (engine.proto BookSnapshot stream)
- WS push for trades (ws-gateway)

This phase is not architectural — it's a transport addition. Pure UI shells (`/markets/[id]` order book) get their first live data here.

---

## 5. Risks / gotchas

1. **i64 / BigInt boundary.** `txn_id` and `*_minor_units` are i64 server-side. `JSON.parse` → number loses precision past 2^53. ts-sdk MUST parse these as `bigint`. Affects every wallet response shape; document in Phase 1 conventions.
2. **HTTP vs gRPC duality.** wallet-ledger ships HTTP; engine.proto declares gRPC. Don't unify the runtime prematurely. Two transport adapters in `runtime/`, one shared `ErrorEnvelope` decoder.
3. **Idempotency key generation.** Post-transaction requires client-supplied `idempotency_key`. Generate via UUID-v7 (time-ordered) so server-side dedupe windows behave; document in `runtime/idempotency.ts`.
4. **Drift between api.rs serde and proto.** Today the HTTP API in `api.rs` is hand-written serde; `ledger.proto` is a parallel gRPC contract. The two will drift unless one of them becomes the source of truth. Recommend: ledger.proto becomes canonical, api.rs is generated from it (prost-build + a thin axum bridge). Out of scope for ts-sdk plan, but flagged.
5. **CORS / auth.** apps/web is server-side rendered; calls from RSC don't need CORS. Calls from `'use client'` components do — api-gateway must allow `apps/web` origin. `TradePanel` is a client component, so once order submission is real this matters.

---

## 6. Recommended next moves

In rough order:

1. **Codex side** — start `api-gateway`. Without it, no UI integration can happen because every UI call wants to hit api-gateway, not raw wallet-ledger.
2. **Tech Lead** — extract / write `ledger.proto` so it matches `api.rs` structs, then regenerate. Avoids HTTP-vs-gRPC drift.
3. **ts-sdk Phase 1** — build the runtime against a local wallet-ledger. No production wiring yet.
4. **First live integration** — `/topup` BalanceBlock once api-gateway is up.

Estimated `apps/web` work for full backend wire-up after Codex catches up to all services: ~1 week (mostly mock→client swap; types and shapes already match by design).

---

## 7. Open questions for next session with Codex

- Is `api.rs` going to keep being the HTTP entry point, or will it be replaced by api-gateway routing?
- gRPC stubs vs HTTP stubs — which gets the canonical `ledger.proto` treatment first?
- WS gateway plan: gRPC streaming vs raw WebSocket vs Cloudflare Durable Objects?
- Auth layer: where does JWT verify happen (api-gateway middleware vs each service)? This determines where ts-sdk attaches the Authorization header.
