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

## Frontend

Run the player web and admin apps together:

```powershell
rtk pnpm dev
```

With the checked-in `.env.example` defaults:

- Web: `http://localhost:3000`
- Admin: `http://localhost:3010`

To run only one app:

```powershell
rtk pnpm --filter @prophetica/web dev
rtk pnpm --filter @prophetica/admin dev
```

## Backend Services

The `core-api` binary is the local bundle for the MVP API surface:

```powershell
rtk cargo run -p core-api --locked
```

It starts these HTTP services from one process:

- api-gateway: `http://localhost:8080`
- user-service: `http://localhost:8084`
- kyc-service: `http://localhost:8085`
- market-service: `http://localhost:8087`
- position-service: `http://localhost:8089`
- referral-service: `http://localhost:8092`

The order path also needs wallet ledger and matching engine processes:

```powershell
rtk cargo run -p wallet-ledger-service-binary --bin wallet-ledger-service --locked
rtk cargo run -p matching-engine-cold --locked
```

Relevant local env vars:

```dotenv
API_GATEWAY_BASE_URL=http://localhost:8080
KYC_SERVICE_BASE_URL=http://localhost:8085
POSITION_SERVICE_BASE_URL=http://localhost:8089
MATCHING_ENGINE_BASE_URL=http://localhost:8088
WALLET_LEDGER_BASE_URL=http://localhost:8086
POSITION_SERVICE_STORE=memory
```

Set `POSITION_SERVICE_STORE=postgres` to read positions from `DATABASE_URL` (or
`POSITION_SERVICE_DATABASE_URL`) and run the position-service migrations. Dev
defaults stay in-memory so the local stack can start without a database-backed
projection.

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
