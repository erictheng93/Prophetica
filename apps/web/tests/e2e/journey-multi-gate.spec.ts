import { expect, test, type Page } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  setupJourneyMocks,
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

  // Start as a visitor and complete the same sign-in step the happy-path
  // covers; the focus of this spec is what happens AFTER login when both
  // KYC and funds are missing.
  await signInFromMarket(page, marketReturnPath, encodedMarketReturnPath);

  // Gate 1: KYC. Tier 0 → /kyc/tier-1?return_to=…
  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/kyc/tier-1\\?return_to=${encodedMarketReturnPath}`),
  );
  await completeTierOne(page);
  await expect(page).toHaveURL(
    new RegExp(`/markets/${JOURNEY_MARKET_ID}\\?side=yes&qty=10$`),
  );

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

async function signInFromMarket(
  page: Page,
  marketReturnPath: string,
  encodedMarketReturnPath: string,
): Promise<void> {
  await page.context().clearCookies();
  await page.goto(marketReturnPath, { waitUntil: "domcontentloaded" });
  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/auth/sign-in\\?return_to=${encodedMarketReturnPath}`),
  );
  await page.locator('input[name="identifier"]').fill("journey@example.com");
  await page
    .locator('input[name="password"]')
    .fill("correct horse battery staple");
  await page.getByRole("button", { name: "登入" }).click();
  await expect(page).toHaveURL(
    new RegExp(`/markets/${JOURNEY_MARKET_ID}\\?side=yes&qty=10$`),
  );
}

async function completeTierOne(page: Page): Promise<void> {
  await page.locator('input[name="legalName"]').fill("Journey User");
  await page.locator('input[name="birthday"]').fill("1990-01-02");
  await page.locator('select[name="country"]').selectOption("TW");
  await page.locator('input[name="documentId"]').fill("A123456789");
  await page.locator('input[name="consent"]').check();
  await page.getByRole("button", { name: "送出基礎驗證" }).click();
}
