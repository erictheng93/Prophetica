# Local Development Runbook

This runbook is the preferred path for running the MVP player web app, admin
app, and core backend services locally.

## Hostnames

Use `http://localhost` for browser-facing Next.js development URLs. Avoid
opening the app at `http://127.0.0.1` during Next dev testing; mixing hostnames
can split cookie origins and dev asset origins, which shows up as hydration or
resource loading issues.

Server-to-server Rust calls still work with either hostname, but the checked-in
local defaults now use `localhost` to keep browser, BFF, and service URLs
aligned.

## Bootstrap

```powershell
rtk pnpm install --frozen-lockfile
if (!(Test-Path .env)) { Copy-Item .env.example .env }
rtk make dev-infra
```

`dev-infra` starts local dependencies such as PostgreSQL, Redis, Kafka, Vault,
Mailhog, and observability services. Stop them with:

```powershell
rtk make down
```

## Full MVP Stack

Run the player web app, admin app, and core API bundle together:

```powershell
rtk pnpm dev
```

With the checked-in `.env.example` defaults:

- api-gateway: `http://localhost:8080`
- wallet-ledger-service: `http://localhost:8086`
- Web: `http://localhost:3000`
- Admin: `http://localhost:3010`

The root dev script loads `.env` from the repository root. If `.env` is missing,
it falls back to `.env.example`, which is enough for local development defaults.
The script also exports `API_GATEWAY_BASE_URL`, `WALLET_LEDGER_BASE_URL`, and
`WALLET_LEDGER_DATABASE_URL` to the child processes when they are not already
set.

To run only the frontend apps:

```powershell
rtk pnpm dev:apps
```

To run only one frontend app:

```powershell
rtk pnpm --filter @prophetica/web dev
rtk pnpm --filter @prophetica/admin dev
```

## Backend Services

The `core-api` binary is the local bundle for the MVP API surface:

```powershell
rtk pnpm dev:core-api
```

Use `rtk cargo run -p core-api --locked` only when the needed env vars are
already exported in the shell; Cargo does not load the root `.env` file.

It starts these HTTP services from one process:

- api-gateway: `http://localhost:8080`
- user-service: `http://localhost:8084`
- kyc-service: `http://localhost:8085`
- wallet-ledger-service: `http://localhost:8086`
- market-service: `http://localhost:8087`
- position-service: `http://localhost:8089`
- referral-service: `http://localhost:8092`

`core-api` runs wallet-ledger migrations on startup using
`WALLET_LEDGER_DATABASE_URL` or `DATABASE_URL`. That makes `/portfolio` able to
show live wallet balances through web BFF -> api-gateway -> wallet-ledger
without a separately started ledger process.

The order path still needs a matching engine process:

```powershell
rtk cargo run -p matching-engine-cold --locked
```

Relevant local env vars:

```dotenv
DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica
API_GATEWAY_DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica
WALLET_LEDGER_DATABASE_URL=postgres://proph:dev@localhost:5432/prophetica
API_GATEWAY_BASE_URL=http://localhost:8080
KYC_SERVICE_BASE_URL=http://localhost:8085
POSITION_SERVICE_BASE_URL=http://localhost:8089
MATCHING_ENGINE_BASE_URL=http://localhost:8088
WALLET_LEDGER_BASE_URL=http://localhost:8086
POSITION_SERVICE_STORE=memory
```

`api-gateway` uses `API_GATEWAY_DATABASE_URL` (or `DATABASE_URL`) for durable
order ownership and the trade-fill saga/outbox retry table. Leaving both unset
falls back to in-memory state in local dev only; production fails closed.

Set `POSITION_SERVICE_STORE=postgres` to read positions from `DATABASE_URL` (or
`POSITION_SERVICE_DATABASE_URL`) and run the position-service migrations. Dev
defaults stay in-memory so the local stack can start without a database-backed
projection.

## Portfolio Live Wallet Check

After `rtk make dev-infra` and `rtk pnpm dev`, check these endpoints:

```powershell
curl.exe -fsS http://localhost:8080/healthz/ready
curl.exe -fsS http://localhost:8086/healthz/ready
```

If `/portfolio` still shows the fallback wallet state, check that:

- `rtk pnpm dev` was used instead of `rtk pnpm dev:apps`.
- Postgres is running from `rtk make dev-infra`.
- `WALLET_LEDGER_BASE_URL` points to `http://localhost:8086`.
- `API_GATEWAY_BASE_URL` points to `http://localhost:8080`.

## Windows Rust Builds

Native Windows Rust builds that touch crates depending on `openssl-sys` require
OpenSSL discovery to be configured. Use one of these paths:

- Install OpenSSL and set `OPENSSL_DIR`.
- Install vcpkg, install OpenSSL for the target triplet, and set `VCPKG_ROOT`.
- Use WSL2 or the Linux Docker build path for Rust validation.

Docker validation example from PowerShell:

```powershell
rtk docker run --rm `
  -e CARGO_TARGET_DIR=/work/target/docker-codex `
  -v "${PWD}:/work" `
  -w /work `
  prophetica-rust-builddeps:codex `
  cargo test -p api-gateway --lib --locked
```

## Current MVP Gaps

- Portfolio positions are now read through api-gateway auth, and
  `position-service` supports `POSITION_SERVICE_STORE=postgres` plus an
  internal fill projection endpoint. Remaining gap: feed it from durable
  matching-engine trade events, including maker-side fills and settlement
  updates.
- Order placement now reaches KYC, wallet precheck, ledger `order_lock`,
  matching-engine proxy, taker aggregate fill projection, and unlock-on-engine
  failure in tests. Remaining gap: expose per-fill trade events from matching
  engine and post maker/taker settlement ledger entries as one durable saga.
