# PoC-B Linux / CI TODO

> Owner: SRE / Tech Lead  
> Status: Pending  
> Blocked by SumSub credentials only for real iframe / webhook / mobile E2E checks. Linux CI can verify Pages adapter output and non-SumSub runtime behavior first.

## Goal

Run PoC-B on Linux or CI to separate Windows `@cloudflare/next-on-pages` tooling failure from real Cloudflare Pages Edge runtime incompatibility.

## Preconditions

- Linux runner with Node / pnpm matching repo toolchain.
- Cloudflare Pages preview project available: `prophetica-web-poc`.
- Non-secret Pages vars may be configured in `apps/web/wrangler.toml`.
- Real SumSub scenarios remain blocked until SPEC-000 Q-9 credentials exist.

## TODO

- [ ] Run workspace install on Linux.
  - Command: `rtk pnpm install --frozen-lockfile`
  - Evidence to capture: pnpm version, lockfile status, install exit code.

- [ ] Run `apps/web` production build.
  - Command: `rtk pnpm --filter @prophetica/web build`
  - Evidence to capture: build exit code and any Edge runtime / Server Action warnings.

- [ ] Run Cloudflare Pages adapter generation on Linux.
  - Command: `rtk pnpm --filter @prophetica/web exec next-on-pages`
  - Evidence to capture: whether `apps/web/.vercel/output/static` exists after the command.
  - If this fails on Linux, treat as real adapter/toolchain blocker and open fallback ADR.

- [ ] Deploy or preview the generated Pages output.
  - Command: `rtk pnpm --filter @prophetica/web exec wrangler pages deploy .vercel/output/static --project-name prophetica-web-poc`
  - Evidence to capture: preview URL, deploy ID, runtime logs.

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
