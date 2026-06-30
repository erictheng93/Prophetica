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

## Shared Rust Build Resources

This repository uses shared Rust build resources on local development machines:

- `sccache` is shared across sessions and agents.
- Cargo artifacts are written to a shared target directory, currently under
  `/Users/eric/.cargo/target` unless the environment explicitly overrides
  `CARGO_TARGET_DIR`.

Treat those resources as shared infrastructure. A broad `cargo test`, especially
for dependency-heavy crates such as `api-gateway`, can occupy compiler/linker
processes, mutate shared incremental state, and slow or interfere with other
active work. Before running a Rust command that may compile many targets, prefer
the narrowest command that proves the touched behavior:

```bash
rtk cargo check -p <crate> --lib --locked
rtk cargo test -p <crate> --lib <test_name> --locked
rtk cargo test -p <crate> --test <integration_test> <test_name> --locked
```

Do not start a whole-crate Rust test such as `rtk cargo test -p api-gateway
--locked` on a shared local machine unless the user explicitly approves the
shared-cache / shared-target impact or no narrower validation can prove the
change. If a broad Rust command is accidentally started, check for remaining
`cargo`, `rustc`, linker, and `sccache` child processes and stop only the
processes from that command before continuing.

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

Full workspace Rust verification is allowed on the local development machine
when cross-crate confidence is needed:

```bash
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo nextest run --workspace --locked
rtk cargo build --workspace --release
rtk pnpm build
```

Use targeted checks for quick iteration, and use the full workspace commands
before declaring broad cross-crate Rust changes fully verified.

## Heavy Verification Locally Or In CI

For Rust-heavy changes, run full verification on the local development machine
or in CI with a persistent checkout and warmed Cargo cache.

On the development machine:

```bash
rtk git checkout -b feat/<topic>
rtk git add <files>
rtk git commit -m "<type(scope): summary>"
rtk git push -u origin feat/<topic>
```

Locally or on the CI/build host:

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

# local machine or CI/build host
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

PR descriptions should list which commands ran locally and which ran in CI. If
heavy workspace verification was intentionally skipped, state where it will run
before merge.
