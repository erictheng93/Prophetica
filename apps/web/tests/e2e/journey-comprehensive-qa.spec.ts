import { expect, test } from "@playwright/test";
import {
  completeTierOne,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  setupJourneyMocks,
  submitTrade,
  type JourneyMocks,
  type JourneyMockState,
} from "./_helpers/journey-mocks.js";

test.describe("Comprehensive Headless Chrome Exploratory QA", () => {
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

  test("full user journey across discovery, auth, kyc, trading, portfolio, responsible gaming, redemption, and support", async ({
    page,
  }) => {
    const marketPath = `/markets/${JOURNEY_MARKET_ID}`;

    // 1. Landing & Market Discovery
    await page.goto("/");
    await expect(page).toHaveTitle(/Prophetica/);
    await expect(page.locator("body")).toBeVisible();

    await page.goto("/markets");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/全市場|市場列表|Market/i).first()).toBeVisible();

    // 2. Market Detail & Orderbook
    await page.goto(marketPath);
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/54|55/).first()).toBeVisible();

    // 3. Unauth Trade Intercept -> Sign In
    await submitTrade(page);
    await expect(page).toHaveURL(new RegExp(`/auth/sign-in\\?return_to=`));
    await expect(page.getByRole("button", { name: "登入" })).toBeVisible();

    // Fill login credentials
    await page.locator('input[name="identifier"]').fill("journey-user@example.com");
    await page.locator('input[name="password"]').fill("ValidPassword123!");
    await page.getByRole("button", { name: "登入" }).click();

    // Auto-return to market page
    await expect(page).toHaveURL(new RegExp(marketPath));

    // 4. KYC Intercept -> Submit Tier 1 KYC
    await submitTrade(page);
    await expect(page).toHaveURL(new RegExp(`/kyc/tier-1\\?return_to=`));
    await completeTierOne(page);

    // Auto-return to market page after passing KYC
    await expect(page).toHaveURL(new RegExp(marketPath));

    // 5. Balance Intercept -> Top-up Assistance
    await submitTrade(page);
    await expect(page).toHaveURL(new RegExp(`/topup\\?return_to=`));
    await expect(page.getByText(/儲值|Top-up|Balance/i).first()).toBeVisible();

    // Simulate funding the account
    state.balanceMinor = 250_000;
    await page.goto(marketPath);

    // 6. Execute Trade Order
    await submitTrade(page);
    await expect(page.getByText(/已送出|已成交|委託成功|accepted|ord-journey-1/i).first()).toBeVisible();

    // 7. Portfolio Inspection
    await page.goto("/portfolio");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/資產總覽|投資組合|Portfolio|點數/i).first()).toBeVisible();

    // 8. Account & Affiliate Flow
    await page.goto("/account/affiliate");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/推廣|推薦|Affiliate|邀請/i).first()).toBeVisible();

    // 9. Responsible Gaming Flow
    await page.goto("/responsible-gaming");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/你的節奏，由你掌握|自我設限|快照/i).first()).toBeVisible();

    await page.goto("/responsible-gaming/limits");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/月儲值上限|目前的月儲值上限/i).first()).toBeVisible();

    await page.goto("/responsible-gaming/cooldown");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/選擇冷靜期長度|冷靜期/i).first()).toBeVisible();

    await page.goto("/responsible-gaming/self-exclusion");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/這是一個很大的決定|解除賬號/i).first()).toBeVisible();

    // Test self-exclusion form submission (timeout 6 months)
    const timeoutForm = page.locator("form").filter({ hasText: /Time-out/i }).first();
    if (await timeoutForm.isVisible()) {
      await timeoutForm.locator('input[name="consent"]').check();
      await timeoutForm.getByRole("button", { name: "啟動 Time-out" }).click();
      await expect(page).toHaveURL(/self-exclusion\?selfExclusion=/);
    }

    // 10. Redemption Flow
    await page.goto("/redemption");
    await expect(page.locator("body")).toBeVisible();
    await expect(page.getByText(/點數兌換|Points redemption/i).first()).toBeVisible();

    // 11. Support & Informational Pages
    await page.goto("/support/faq");
    await expect(page.locator("body")).toBeVisible();

    await page.goto("/legal/terms");
    await expect(page.locator("body")).toBeVisible();

    await page.goto("/legal/privacy");
    await expect(page.locator("body")).toBeVisible();

    await page.goto("/legal/market-rules");
    await expect(page.locator("body")).toBeVisible();
  });
});
