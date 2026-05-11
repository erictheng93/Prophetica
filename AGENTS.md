# Repository Guidelines

## Project Structure & Module Organization

Prophetica is a Rust/pnpm monorepo. Frontend apps live in `apps/` (`web`, `admin`, `liff`). Shared TypeScript packages live in `packages/` (`ui`, `ts-sdk`, `proto`, `design-tokens`). Rust services are in `services/`, with shared crates in `crates/`. Infrastructure is under `infra/`; SQLx metadata is in `.sqlx/`; references are in `docs/`, `design/`, and `specs/`; scripts and mocks are in `tools/`.

## Build, Test, and Development Commands

Use `rtk` before external CLI commands when invoking developer tools from Codex.

- `rtk pnpm dev`: run all Turbo `dev` tasks in parallel.
- `rtk pnpm build`: build all pnpm workspace targets.
- `rtk pnpm lint` / `rtk pnpm typecheck`: run linting and TypeScript checks.
- `rtk pnpm test`: run pnpm workspace tests.
- `rtk cargo nextest run --workspace --locked`: run Rust tests.
- `rtk cargo clippy --workspace --all-targets --locked -- -D warnings`: enforce Rust lints.
- `rtk make dev` / `rtk make down`: start or stop local Docker dependencies.
- `rtk make proto`: regenerate protobuf outputs with Buf.

## Development Compute Policy

See `docs/runbooks/development-compute-policy.md` for the full policy. On
resource-constrained macOS machines, prefer targeted Rust checks such as

- `rtk cargo check -p <crate> --locked`
- `rtk cargo clippy -p <crate> --all-targets --locked -- -D warnings`
- `rtk cargo test -p <crate> --locked`

Do not routinely run heavy workspace commands on weak macOS hardware unless the
user explicitly asks or the machine is the intended validation host. Heavy
commands include:

- `rtk cargo clippy --workspace --all-targets --locked -- -D warnings`
- `rtk cargo nextest run --workspace --locked`
- `rtk cargo build --workspace --release`

For shared crates, `Cargo.toml`/`Cargo.lock`, generated Rust/protobuf/SQLx
artifacts, or cross-service behavior, push the branch and run heavy workspace
verification on a stronger build host or in GitHub Actions. Document which
machine or CI run performed heavy verification.

## Coding Style & Naming Conventions

Follow `.editorconfig`: UTF-8, LF endings, final newline, two-space indentation by default, and four spaces for Rust/TOML. Rust uses edition 2024, forbids `unsafe_code`, and warns on broad Clippy lint groups; avoid `unwrap()` and `expect()` in production paths. Use Prettier for `ts`, `tsx`, `js`, `jsx`, `json`, `md`, and `css`; Tailwind class ordering is handled by `prettier-plugin-tailwindcss`.

Use kebab-case for package and service directories (`wallet-ledger-service`, `design-tokens`). Prefer Rust `snake_case`, TypeScript `camelCase`, and `PascalCase` components/types.

## Testing Guidelines

Place Rust unit tests near code under `#[cfg(test)]` and integration tests in crate or service `tests/` directories. Frontend/package tests should live beside the feature or follow local app conventions. Run both `rtk pnpm test` and `rtk cargo nextest run --workspace --locked` before PRs that touch shared behavior.

## Commit & Pull Request Guidelines

Recent history uses Conventional Commits with scopes, for example `docs(spec): ...` and `docs(spec): ... (BREAKING)`. Use concise imperative subjects: `feat(api-gateway): add auth middleware`, `fix(wallet-ledger-service): handle duplicate event`.

PRs should describe the change, list validation commands run, link relevant issues/specs, and include screenshots for UI changes. Call out migrations, generated files, environment changes, and any follow-up work.

## Security & Configuration Tips

Copy `.env.example` for local configuration and do not commit secrets. Prefer checked-in scripts in `tools/scripts/` for bootstrap, migrations, and seeding. Regenerate protobuf and SQLx artifacts intentionally, and include generated outputs only when required by the workflow.

## RTK Command Preference
When invoking external CLI commands from Codex for this repository, prefer the `rtk` prefix for tools such as `php`, `composer`, `npm`, `git`, and `bash`.

Examples:
- `rtk php artisan test`
- `rtk composer install`
- `rtk npm run build`

PowerShell builtins and simple read-only inspection commands do not need the `rtk` prefix.

