# PoC-B Linux / CI TODO

> Owner: SRE / Tech Lead  
> Status: Pending  
> Blocked by SumSub credentials only for real iframe / webhook / mobile E2E checks. Linux CI can verify OpenNext Workers adapter output and non-SumSub runtime behavior first.

## Goal

Run PoC-B on Linux or CI to verify the OpenNext Cloudflare Workers adapter output separately from local Windows tooling behavior.

## Preconditions

- Linux runner with Node / pnpm matching repo toolchain.
- Cloudflare Workers preview project available: `prophetica-web-poc`.
- Non-secret Workers vars may be configured in `apps/web/wrangler.toml`.
- Real SumSub scenarios remain blocked until SPEC-000 Q-9 credentials exist.

## TODO

- [ ] Run workspace install on Linux.
  - Command: `rtk pnpm install --frozen-lockfile`
  - Evidence to capture: pnpm version, lockfile status, install exit code.
  - Use a native Linux install; do not reuse a Windows `node_modules` tree because OpenNext dependencies include platform-specific native packages.

- [ ] Run `apps/web` production build.
  - Command: `rtk pnpm --filter @prophetica/web build`
  - Evidence to capture: build exit code and any Edge runtime / Server Action warnings.

- [ ] Run OpenNext Cloudflare adapter generation on Linux.
  - Command: `rtk pnpm --filter @prophetica/web exec opennextjs-cloudflare build`
  - Evidence to capture: whether `apps/web/.open-next/worker.js` and `apps/web/.open-next/assets` exist after the command.
  - If this fails on Linux, treat as real adapter/toolchain blocker and open fallback ADR.

- [ ] Preview the generated Workers output.
  - Command: `rtk pnpm --filter @prophetica/web preview`
  - Evidence to capture: local Workers preview URL and runtime logs.

- [ ] Scenario B: verify WebSocket fan-out path against preview or configured gateway.
  - Evidence to capture: HTTP 101 upgrade, monotonic sequence, reconnect + gap behavior.
  - If no gateway endpoint exists, record as infra dependency rather than Edge runtime failure.

- [ ] Scenario C: verify Server Actions / route behavior on preview.
  - Evidence to capture: page render, action invocation result, runtime error logs.
  - If build succeeds but action fails due to Node-only APIs, decide Node.js route fallback or `apps/web-bff/`.

- [ ] Scenario D: verify `fetch` + `AbortController` behavior on preview.
  - Evidence to capture: timeout / abort path, retry behavior, no unhandled runtime exception.

- [ ] Scenario E: verify env binding shape without exposing secrets.
  - Evidence to capture: public var available only where expected; secret names not leaked into client static bundle.
  - Do not place real SumSub secrets in `wrangler.toml`; use Cloudflare secret bindings only after Q-9 is completed.

- [ ] Scenario A partial: verify KYC page shell, CSP, SDK script allowance, and mock-provider fallback on preview.
  - Evidence to capture: KYC route loads, CSP header includes SumSub origins only on `/kyc/*`, no client bundle secret leakage.
  - Real SumSub iframe mount, applicant token, webhook HMAC, P95 flow time, and mobile WebRTC remain blocked by Q-9.

- [ ] Backfill `docs/poc/PRE-FLIGHT-POCS.md` section 4.4 with Linux/CI results.
  - Include pass/fail per scenario, command logs, preview URL, commit SHA, and fallback decision if any.

- [ ] Backfill SPECs after PoC-B result.
  - If all non-SumSub checks pass: keep Edge runtime as provisional path and leave Q-9 as the remaining live E2E blocker.
  - If adapter or runtime fails on Linux: update SPEC-014 section 3.2.1 and open ADR for Node.js runtime / BFF fallback.
