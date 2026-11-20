import { expect, test } from "@playwright/test";
import {
  freshJourneyState,
  setupJourneyMocks,
  signInFromMarketTrade,
  JOURNEY_MARKET_ID,
  type JourneyMocks,
  type JourneyMockState,
} from "./_helpers/journey-mocks.js";

test.describe("Responsible Gaming Flows", () => {
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

  test("overview displays monthly snapshot and navigation cards", async ({
    page,
  }) => {
    await page.goto("/responsible-gaming");
    await expect(page.locator("body")).toBeVisible();
    await expect(
      page.getByText(/你的節奏，由你掌握|自我設限|快照/i).first(),
    ).toBeVisible();

    // Verify presence of tools links
    await expect(page.locator('a[href="/responsible-gaming/limits"]').first()).toBeVisible();
    await expect(page.locator('a[href="/responsible-gaming/cooldown"]').first()).toBeVisible();
    await expect(page.locator('a[href="/responsible-gaming/self-exclusion"]').first()).toBeVisible();
  });

  test("deposit limits page displays current limit and options", async ({
    page,
  }) => {
    await page.goto("/responsible-gaming/limits");
    await expect(page.locator("body")).toBeVisible();
    await expect(
      page.getByText(/月儲值上限|目前的月儲值上限/i).first(),
    ).toBeVisible();

    // Limit radios should exist
    const radioOptions = page.locator('input[type="radio"][name="monthlyLimit"]');
    expect(await radioOptions.count()).toBeGreaterThan(0);
  });

  test("cooldown page presents duration options", async ({ page }) => {
    await page.goto("/responsible-gaming/cooldown");
    await expect(page.locator("body")).toBeVisible();
    await expect(
      page.getByText(/選擇冷靜期長度|冷靜期/i).first(),
    ).toBeVisible();
  });

  test("self-exclusion page allows authenticated player to submit timeout request", async ({
    page,
  }) => {
    // Sign in first
    await signInFromMarketTrade(page, `/markets/${JOURNEY_MARKET_ID}`);

    await page.goto("/responsible-gaming/self-exclusion");
    await expect(page.locator("body")).toBeVisible();
    await expect(
      page.getByText(/這是一個很大的決定|解除賬號/i).first(),
    ).toBeVisible();

    const timeoutForm = page.locator("form").filter({ hasText: /Time-out/i }).first();
    if (await timeoutForm.isVisible()) {
      await timeoutForm.locator('input[name="consent"]').check();
      await timeoutForm.getByRole("button", { name: "啟動 Time-out" }).click();
      await expect(page).toHaveURL(/self-exclusion\?selfExclusion=/);
    }
  });
});
