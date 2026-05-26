# SPEC-005a KYC provider Frontend PR Notes

## SPEC Refs

- `b1feaee feat(kyc): close SPEC-005 §0.2 B1/B3 backend hardening`
  - SPEC-005 §0.2 B1/B3
- `1775641 feat(specs): add SPEC-005a KYC provider frontend integration contract`
  - SPEC-005a §2 / §5.2.1 / §6.1 / §7.3 / §8 / §10
- `159cc98 feat(ts-sdk): add KycIdentityProvider abstraction with mock + selector`
  - SPEC-005a §2 / §10
- `73003c8 feat(web): wire Tier 2/3 KYC state machine and provider launch flow`
  - SPEC-005a §2 / §6.1 / §10
- `0d5e0cd feat(web): wire Tier 3 polling and SPEC-005a §6.1 fail-soft handling`
  - SPEC-005a §6.1 / §10
- `14e9d4c fix(kyc): SPEC-005a §5.2.1 enforce server-derived userId end-to-end`
  - SPEC-005a §5.2.1
- `0ba71cb feat(web): SPEC-005a §8 add KYC provider CSP scoped to /kyc/* routes`
  - SPEC-005a §8
- `a7095c9 feat(web): SPEC-005a §6.1 + §2 lock down polling timing and KYC provider locale`
  - SPEC-005a §2 / §6.1
- `c67e5ef feat(web): SPEC-005a §7.3 KYC telemetry - 5 metrics + Prom exposition`
  - SPEC-005a §7.3
- `1159fee test(kyc): SPEC-005a §10 close acceptance gates`
  - SPEC-005a §10
- `4bb8c51 fix(kyc): include tier one context in upgrade keys`
  - SPEC-005a §5.3
- `2798b8e fix(kyc): pass verified country to provider launch`
  - SPEC-005a §2 / §5.2.1

## Merge Governance

- This is intentionally stacked on `feat/spec-008-dr-gap-fill` until SPEC-008 lands.
- After SPEC-008 is merged, retarget this PR to `main` and re-run the web, ts-sdk, and kyc-service validation commands.

## Validation

- `rtk pnpm --filter @prophetica/ts-sdk test`
- `rtk pnpm --filter @prophetica/web test`
- `rtk cargo nextest run -p kyc-service --locked`
