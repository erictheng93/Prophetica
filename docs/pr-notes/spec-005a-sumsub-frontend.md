# SPEC-005a SumSub Frontend PR Notes

## SPEC Refs

- `b0cf7da feat(kyc): close SPEC-005 §0.2 B1/B3 backend hardening`
  - SPEC-005 §0.2 B1/B3
- `6cf41df feat(specs): add SPEC-005a SumSub frontend integration contract`
  - SPEC-005a §2 / §5.2.1 / §6.1 / §7.3 / §8 / §10
- `45b1952 feat(ts-sdk): add KycIdentityProvider abstraction with mock + selector`
  - SPEC-005a §2 / §10
- `377d395 feat(web): wire Tier 2/3 KYC state machine and provider launch flow`
  - SPEC-005a §2 / §6.1 / §10
- `f3e0662 feat(web): wire Tier 3 polling and SPEC-005a §6.1 fail-soft handling`
  - SPEC-005a §6.1 / §10
- `38fda40 fix(kyc): SPEC-005a §5.2.1 enforce server-derived userId end-to-end`
  - SPEC-005a §5.2.1
- `e3ebb06 feat(web): SPEC-005a §8 add SumSub CSP scoped to /kyc/* routes`
  - SPEC-005a §8
- `8ab6e55 feat(web): SPEC-005a §6.1 + §2 lock down polling timing and SumSub locale`
  - SPEC-005a §2 / §6.1
- `2249558 feat(web): SPEC-005a §7.3 KYC telemetry - 5 metrics + Prom exposition`
  - SPEC-005a §7.3
- `fd91386 test(kyc): SPEC-005a §10 close acceptance gates`
  - SPEC-005a §10
- `28877ae fix(kyc): include tier one context in upgrade keys`
  - SPEC-005a §5.3

## Merge Governance

- This is intentionally stacked on `feat/spec-008-dr-gap-fill` until SPEC-008 lands.
- After SPEC-008 is merged, retarget this PR to `main` and re-run the web, ts-sdk, and kyc-service validation commands.

## Validation

- `rtk pnpm --filter @prophetica/ts-sdk test`
- `rtk pnpm --filter @prophetica/web test`
- `rtk cargo nextest run -p kyc-service --locked`
