import { expect, test } from "@playwright/test";
import {
  freshJourneyState,
  setupJourneyMocks,
  signInFromMarketTrade,
  JOURNEY_MARKET_ID,
  type JourneyMocks,
  type JourneyMockState,
} from "./_helpers/journey-mocks.js";

test.describe("Points Redemption Flow", () => {
  let mocks: JourneyMocks | undefined;
  let state: JourneyMockState;

  test.beforeEach(async () => {
    state = freshJourneyState();
    mocks = await setupJourneyMocks(state);
  });

  test.afterEach(async () => {
    await mocks?.userService.close();
    await mocks?.apiGateway.close();
    await mocks?.kycService.close();
  });

  test("authenticated player can view catalog and submit redemption order", async ({
    page,
  }) => {
    // 1. Sign in first
    await signInFromMarketTrade(page, `/markets/${JOURNEY_MARKET_ID}`);

    // 2. Navigate to /redemption
    await page.goto("/redemption");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText("Points redemption")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Catalog" })).toBeVisible();

    // 3. Verify catalog items
    const createOrderBtn = page.getByRole("button", { name: "Create order" }).first();
    await expect(createOrderBtn).toBeVisible();

    // 4. Click create order
    await createOrderBtn.click();

    // 5. Verify order is created or status is updated
    await expect(
      page.getByText(/Order created|Active entitlements|Order lookup/i).first(),
    ).toBeVisible();
  });
});
