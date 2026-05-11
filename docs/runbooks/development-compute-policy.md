# Development Compute Policy

This project is a Rust/pnpm monorepo. Rust workspace verification can be
resource-heavy, so developers should separate the local edit loop from full
workspace validation.

## Terms

- Compile: Rust compiler work. `cargo check`, `cargo clippy`, `cargo test`, and
  `cargo build` can all compile code, but with different cost.
- Build: produce deployable or runnable artifacts. Examples: `cargo build`,
  `cargo build --release`, and `pnpm build`.
- Test: compile test targets and execute tests. Tests validate behavior; they are
  not the same as builds, even though they often compile code first.

Typical Rust cost order:

```text
cargo check < cargo clippy < cargo test / cargo nextest < cargo build --release
```

Actual cost depends on cache state, changed crates, enabled targets, and whether
the command covers one package or the whole workspace.

## Local Development On A Resource-Constrained Mac

Use the weaker macOS machine for editing and targeted checks. Prefer package- or
feature-scoped commands:

```bash
rtk cargo fmt --all -- --check
rtk cargo check -p <crate> --locked
rtk cargo clippy -p <crate> --all-targets --locked -- -D warnings
rtk cargo test -p <crate> --locked
rtk cargo test -p <crate> --test <integration_test> --locked
rtk pnpm typecheck
rtk pnpm lint
rtk pnpm test
```

Avoid running these routinely on weak macOS hardware:

```bash
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo nextest run --workspace --locked
rtk cargo build --workspace --release
rtk pnpm build
```

Run the heavy commands locally only when the developer explicitly chooses to
spend the resources or when the local machine is the intended validation host.

## Heavy Verification On A Build Host

For Rust-heavy changes, prefer a stronger Linux/Windows/macOS build host with a
persistent checkout and warmed Cargo cache.

On the development machine:

```bash
rtk git checkout -b feat/<topic>
rtk git add <files>
rtk git commit -m "<type(scope): summary>"
rtk git push -u origin feat/<topic>
```

On the build host:

```bash
rtk git fetch origin
rtk git checkout feat/<topic>
rtk git pull --ff-only
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo nextest run --workspace --locked
```

If the branch already exists on both machines, the steady-state loop is:

```bash
# development machine
rtk git push

# build host
rtk git pull --ff-only
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo nextest run --workspace --locked
```

Use GitHub pull request CI when a shared, auditable validation result is needed.
Do not rely on a feature-branch push alone unless the workflow is configured to
run on that event.

## When To Run What

Run targeted Rust checks when:

- A change is limited to one Rust crate or service.
- You need fast syntax, type, borrow-check, or lint feedback.
- You changed unit-level behavior and can name the affected crate/test.

Run workspace Rust checks on a build host or CI when:

- A shared crate under `crates/` changed.
- `Cargo.toml`, `Cargo.lock`, feature flags, or toolchain files changed.
- Generated Rust, SQLx metadata, protobuf outputs, or cross-service contracts changed.
- Multiple services are touched.
- The change is ready for merge.

Run pnpm typecheck/lint/test when:

- TypeScript, React, shared packages, app routes, or frontend SDK code changed.

Run pnpm build when:

- Next.js build behavior, deployment config, Cloudflare/OpenNext config, or build
  tooling changed.
- Preparing a release or validating a deployment artifact.

Run full release builds only when:

- Producing artifacts for deployment.
- Validating release-specific compiler behavior.
- Investigating build-only failures.

## PR Expectations

PR descriptions should list which commands ran locally and which ran on the
build host or CI. If heavy workspace verification was intentionally skipped,
state where it will run before merge.
