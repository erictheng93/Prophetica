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

// SPEC-023 plan Task 4.5 — when a freshly-authenticated user lacks both
// KYC tier and funds, the journey escorts them through each gate in turn.
// Every intercept preserves `return_to` pointing to the original deep-link.

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

test("trade with no KYC and no funds clears one gate at a time, preserving return_to", async ({
  page,
}) => {
  const marketReturnPath = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10`;
  const encodedMarketReturnPath = encodeURIComponent(marketReturnPath);

  // Sign in as a visitor (state: TIER_0, balance 0). Focus here is what
  // happens AFTER login when both gates are unmet.
  await signInFromMarketTrade(page, marketReturnPath);

  // Gate 1: KYC. Tier 0 → /kyc/tier-1?return_to=…
  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/kyc/tier-1\\?return_to=${encodedMarketReturnPath}`),
  );
  await completeTierOne(page);
  await expect(page).toHaveURL(pathToRegExp(marketReturnPath));

  // Gate 2: funds. Balance still 0 → /topup?return_to=…
  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/topup\\?return_to=${encodedMarketReturnPath}`),
  );

  // Top-up MVP is service-assisted — operator credits balance via
  // SPEC-002 manual adjustment. Simulate that here.
  state.balanceMinor = 10_000;
  await page.goto(marketReturnPath, { waitUntil: "domcontentloaded" });

  // All gates clear — order goes through.
  await submitTrade(page);
  await expect.poll(() => state.orderBody).toMatchObject({
    side: "buy",
    outcome: "YES",
    quantity: 10,
  });
});
