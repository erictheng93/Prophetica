import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  completeTierOne,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  pathToRegExp,
  setupJourneyMocks,
  signInFromMarketTrade,
  submitTrade,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

// SPEC-023 plan Task 4.3 — isolate the KYC gate. The user is funded but
// below Tier 1, so ONLY /kyc/tier-1 should fire — never /topup. After
// clearing KYC the order goes through without a funds detour.

const state: JourneyMockState = freshJourneyState();
let mocks: JourneyMocks;

test.beforeAll(async () => {
  mocks = await setupJourneyMocks(state);
});

test.beforeEach(() => {
  Object.assign(state, freshJourneyState());
});

test.afterAll(async () => {
  await closeJourneyMocks(mocks);
});

test("KYC-only gate intercepts to /kyc/tier-1 and never to /topup", async ({
  page,
}) => {
  const marketReturnPath = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10`;
  const encodedMarketReturnPath = encodeURIComponent(marketReturnPath);

  // Funds are sufficient from the start; only KYC is missing. The order
  // soft-gate (price 55 × qty 10 × 1.02 = 561 minor units) is well under
  // this balance, so a funds detour would be a bug.
  state.kycTier = "TIER_0";
  state.balanceMinor = 10_000;

  await signInFromMarketTrade(page, marketReturnPath);

  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/kyc/tier-1\\?return_to=${encodedMarketReturnPath}`),
  );

  await completeTierOne(page);
  await expect(page).toHaveURL(pathToRegExp(marketReturnPath));

  // KYC cleared and funds were never the blocker — the next attempt
  // places the order with no /topup hop in between.
  await submitTrade(page);
  await expect.poll(() => state.orderBody).toMatchObject({
    side: "buy",
    outcome: "YES",
    quantity: 10,
  });
});
