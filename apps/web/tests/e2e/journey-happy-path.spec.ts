import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  JOURNEY_USER_SERVICE_SESSION,
  setupJourneyMocks,
  submitTrade,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

let state: JourneyMockState = freshJourneyState();
let mocks: JourneyMocks;

test.beforeAll(async () => {
  mocks = await setupJourneyMocks(state);
});

test.beforeEach(() => {
  // Reuse the same object reference so the mock handlers continue to see
  // current values, but reset its fields between scenarios.
  Object.assign(state, freshJourneyState());
});

test.afterAll(async () => {
  await closeJourneyMocks(mocks);
});

test("visitor can complete sign-in, KYC, top-up assistance, and order journey", async ({
  page,
}) => {
  const marketReturnPath = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10`;
  const encodedMarketReturnPath = encodeURIComponent(marketReturnPath);

  await page.goto(marketReturnPath, { waitUntil: "domcontentloaded" });
  await expect(page.locator("aside")).toHaveAttribute(
    "data-journey-status",
    "visitor",
  );

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

  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/kyc/tier-1\\?return_to=${encodedMarketReturnPath}`),
  );

  await page.locator('input[name="legalName"]').fill("Journey User");
  await page.locator('input[name="birthday"]').fill("1990-01-02");
  await page.locator('select[name="country"]').selectOption("TW");
  await page.locator('input[name="documentId"]').fill("A123456789");
  await page.locator('input[name="consent"]').check();
  await page.getByRole("button", { name: "送出基礎驗證" }).click();
  await expect(page).toHaveURL(
    new RegExp(`/markets/${JOURNEY_MARKET_ID}\\?side=yes&qty=10$`),
  );

  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/topup\\?return_to=${encodedMarketReturnPath}`),
  );

  state.balanceMinor = 10_000;
  await page.goto(`/markets/${JOURNEY_MARKET_ID}`, {
    waitUntil: "domcontentloaded",
  });
  await submitTrade(page);

  await expect.poll(() => state.orderBody).toMatchObject({
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    price_pts: 55,
    quantity: 10,
    time_in_force: "GTC",
  });
  expect(state.orderAuthorization).toBe(`Bearer ${JOURNEY_USER_SERVICE_SESSION}`);
});

test("sign-in rejects cross-origin return_to targets", async ({ page }) => {
  await page.goto("/auth/sign-in?return_to=https://evil.example/x", {
    waitUntil: "domcontentloaded",
  });

  await page.locator('input[name="identifier"]').fill("journey@example.com");
  await page
    .locator('input[name="password"]')
    .fill("correct horse battery staple");
  await page.getByRole("button", { name: "登入" }).click();

  await expect(page).toHaveURL(new RegExp("/portfolio$"));
});
