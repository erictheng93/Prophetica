# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Engineering Posture (operating mode)

Operate as a senior systems architect with full project oversight, not as a per-ticket assistant.

- **Think in systems.** Before changing a file, trace how the change ripples through SPECs, ADRs, implementation plans, code, CI, deploy targets, and other apps. If one moves, check whether others must follow in the same change.
- **Surface trade-offs explicitly.** State what is gained, lost, deferred, and whose decision it is. No silent assumptions.
- **Verify, don't assume.** Read the actual code path before claiming an architecture. The shipped behavior is the source of truth; specs describe intent.
- **Push back when warranted.** Quantify concerns when possible (latency, attack surface, drift cost). False agreement on a flawed approach is a failure mode.
- **Small tasks get a project-wide sanity check.** A CI version bump, a rustfmt commit, or a doc tweak still pulls in the question: "does anything else need to move with this?"

This posture applies to every interaction on this repo, including small tasks. It does not relax the existing rules below — SDD, atomic commits, no-branches-unless-asked, etc., all still hold.

## Spec-Driven Development (non-negotiable)

This repo practices SDD. Every change must trace to a spec section.

- Specs live in `specs/` as HTML files (`SPEC-000-index.html` is the entry point; §5 has the cross-doc decision table).
- PR descriptions **must** cite `SPEC-XXX § N.N` or they are rejected.
- Spec changes go in their own PR, reviewed *before* the implementing code. Specs must never lag behind code.
- Architecture decisions are in `docs/architecture/decisions/` (ADR-0001..0004).

`AGENTS.md` holds the full contributor guide (commit conventions, compute policy, security tips) — read it for anything not covered here.

## Common Commands

The `Makefile` is the canonical entry point; it wraps pnpm/Turborepo and Cargo.

| Command | Action |
|---|---|
| `make dev` (or `pnpm dev`) | MVP only — runs `web` (:3000) + `admin` (:3002) in parallel |
| `pnpm dev:all-apps` | All apps via Turbo, including `liff` and `landing-sports` |
| `make dev-infra` / `make down` | Start / tear down local Docker deps (Postgres, Redis, Kafka, Vault, Mailhog) |
| `make test` | `turbo run test` + `cargo nextest run --workspace --locked` |
| `make lint` | `turbo run lint` + `cargo clippy --workspace ... -D warnings` + `cargo fmt --check` |
| `make fmt` | `cargo fmt --all` + `turbo run format` |
| `make proto` | Regenerate protobuf stubs via Buf |
| `make doctor` / `make bootstrap` | Version checks / first-time setup |
| `make migrate` / `make seed` | DB migrations and seed data |

### Targeted runs (prefer these — see Compute Policy below)

- Single Rust crate: `cargo check -p <crate> --locked`, `cargo clippy -p <crate> --all-targets --locked -- -D warnings`, `cargo nextest run -p <crate> --locked`
- Single Rust test: `cargo nextest run -p <crate> <test_name_substring>`
- Single frontend app: `pnpm --filter @prophetica/web <lint|test|build>`
- `pnpm validate:landing-variants` — validates landing variant manifests (also enforced in CI)

### Per-app script gotchas

App `package.json` scripts are non-standard: `lint` runs **`tsc`** (a typecheck via `tsconfig.lint.json`), not ESLint. `test` compiles with `tsconfig.test.json` then runs Node's built-in test runner (`node --test`). There is no Vitest/Jest.

## Architecture

Rust/pnpm monorepo. Backend is a Cargo workspace (`Cargo.toml`); frontend is pnpm 9 + Turborepo (`pnpm-workspace.yaml`, `turbo.json`).

### Backend — the `services/` vs `services/_binaries/` split

This is the key non-obvious structure. `services/*` (e.g. `api-gateway`, `matching-engine`, `wallet-ledger-service`) are **library crates** holding domain logic. `services/_binaries/*` (e.g. `core-api`, `realtime-gateway`, `matching-engine-hot`) are the thin **deployable binaries** that compose one or more service crates into a runnable process. When adding behavior, put logic in the service crate; wire it into a binary only at the composition layer.

- Shared crates: `crates/sdk`, `crates/proto`.
- Rust edition 2024, toolchain 1.85, `unsafe_code` forbidden. Clippy `all` + `pedantic` warn; `unwrap_used`/`expect_used` warn — avoid `unwrap()`/`expect()` on production paths.
- SQLx runs in **offline mode** — query metadata is committed in `.sqlx/` and CI sets `SQLX_OFFLINE=true`. Regenerate it intentionally when queries change.

### Protobuf pipeline

`packages/proto/v1/*.proto` is the single source of truth. `buf generate` (`make proto`) emits **committed** stubs into `crates/proto` (Rust) and `packages/proto/gen` (TypeScript). Edit the `.proto` files, then regenerate — never hand-edit generated output.

### Frontend

Four Next.js 16 / React 19 apps in `apps/`: `web` (player, :3000), `admin` (back office, :3002), `liff` (LINE), `landing-sports`. They deploy to Cloudflare via OpenNext (`open-next.config.ts`, `wrangler.toml`). Shared TypeScript packages in `packages/`: `ui`, `ts-sdk`, `design-tokens`, `proto` — consumed as `workspace:*`.

Admin login uses `user-service` local accounts (`admin`/`manager`/`affiliate` @ `prophetica.local`, password = role name).

## Development Compute Policy

Full policy: `docs/runbooks/development-compute-policy.md`. On resource-constrained machines, **do not** routinely run heavy workspace commands (`cargo clippy --workspace`, `cargo nextest run --workspace`, `cargo build --release`). Use the targeted `-p <crate>` forms above. For changes to shared crates, `Cargo.lock`, or generated proto/SQLx artifacts, push the branch and let CI or a stronger host run full verification — and document which host did so.

## Style

- `.editorconfig`: UTF-8, LF, final newline. 2-space indent default; 4-space for Rust and TOML.
- Prettier formats `ts/tsx/js/jsx/json/md/css`; Tailwind classes ordered by `prettier-plugin-tailwindcss`.
- Directory names kebab-case (`wallet-ledger-service`, `design-tokens`).
- Conventional Commits with scopes, imperative subjects (e.g. `feat(api-gateway): add auth middleware`).
- Do not create branches or open PRs unless explicitly asked. Keep commits atomic — never mix unrelated functional, generated, doc, or config changes.
