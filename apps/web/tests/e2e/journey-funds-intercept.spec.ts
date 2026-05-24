import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  setupJourneyMocks,
  signInFromMarketTrade,
  submitTrade,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

// SPEC-023 plan Task 4.4 — isolate the funds gate. The user is already
// Tier 1 but has a zero balance, so ONLY /topup should fire — never
// /kyc/tier-1. After funding the order goes through without a KYC detour.

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

test("funds-only gate intercepts to /topup and never to /kyc/tier-1", async ({
  page,
}) => {
  const marketReturnPath = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10`;
  const encodedMarketReturnPath = encodeURIComponent(marketReturnPath);

  // KYC already cleared; only funds are missing. A /kyc/tier-1 detour
  // here would be a bug.
  state.kycTier = "TIER_1";
  state.balanceMinor = 0;

  await signInFromMarketTrade(page, marketReturnPath);

  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/topup\\?return_to=${encodedMarketReturnPath}`),
  );

  // Operator credits balance via SPEC-002 manual adjustment.
  state.balanceMinor = 10_000;
  await page.goto(marketReturnPath, { waitUntil: "domcontentloaded" });

  await submitTrade(page);
  await expect.poll(() => state.orderBody).toMatchObject({
    side: "buy",
    outcome: "YES",
    quantity: 10,
  });
});
