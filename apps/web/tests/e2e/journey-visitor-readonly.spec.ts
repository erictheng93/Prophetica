import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  setupJourneyMocks,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

// SPEC-023 plan Task 4.6 / INV-JOURNEY-7 — visitors (no proph_session
// cookie) may read public market routes; only state-changing actions
// trigger the §7 intercept. The market list, the market detail page,
// the order book, and the trade-panel rendering must all be reachable
// without auth.

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

test("visitor can browse the public market list without auth", async ({
  page,
}) => {
  await page.context().clearCookies();
  await page.goto("/markets", { waitUntil: "domcontentloaded" });

  // Mocked market list returns one entry titled "Journey E2E Market".
  await expect(
    page.getByRole("heading", { name: "Journey E2E Market" }),
  ).toBeVisible();
});

test("visitor sees market detail, order book, and trade panel without auth", async ({
  page,
}) => {
  await page.context().clearCookies();
  await page.goto(`/markets/${JOURNEY_MARKET_ID}`, {
    waitUntil: "domcontentloaded",
  });

  // Title renders (server-side fetch resolved against the mock).
  await expect(
    page.getByRole("heading", { name: "Journey E2E Market" }),
  ).toBeVisible();

  // Order book cells from the gateway mock: bids at 54¢, asks at 55¢.
  await expect(page.getByText("54¢").first()).toBeVisible();
  await expect(page.getByText("55¢").first()).toBeVisible();

  // Trade panel is rendered but in visitor state — the intercept logic
  // is exercised by journey-unauth-intercept.spec.ts; here we just
  // verify the panel is reachable from a visitor.
  await expect(page.locator("aside")).toHaveAttribute(
    "data-journey-status",
    "visitor",
  );
});

test("visitor browsing does not place an order against the gateway", async ({
  page,
}) => {
  // Defensive — make sure no part of the read-only render hits the
  // POST /orders endpoint of the gateway. A regression that
  // misconfigured the order panel to auto-submit would be caught here.
  await page.context().clearCookies();
  await page.goto(`/markets/${JOURNEY_MARKET_ID}`, {
    waitUntil: "domcontentloaded",
  });
  // Give the SSR + hydration cycle a chance to fire any unintended
  // network calls before checking.
  await page.waitForLoadState("networkidle");

  expect(state.orderBody).toBeUndefined();
  expect(state.orderAuthorization).toBeUndefined();
});
