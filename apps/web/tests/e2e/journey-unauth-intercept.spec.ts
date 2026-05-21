import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  JOURNEY_MARKET_ID,
  setupJourneyMocks,
  submitTrade,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

// SPEC-023 plan Task 4.2 — unauthenticated trade attempts are intercepted
// to /auth/sign-in with the deep-link preserved in `return_to`, and the
// order is never forwarded to the gateway.

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

test("visitor attempting to trade is intercepted to sign-in with same-origin return_to", async ({
  page,
}) => {
  const target = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10`;
  const encoded = encodeURIComponent(target);

  await page.context().clearCookies();
  await page.goto(target, { waitUntil: "domcontentloaded" });
  await expect(page.locator("aside")).toHaveAttribute(
    "data-journey-status",
    "visitor",
  );

  await submitTrade(page);

  await expect(page).toHaveURL(
    new RegExp(`/auth/sign-in\\?return_to=${encoded}`),
  );
  // The trade panel must not have reached the gateway — no order was
  // captured by the mock.
  expect(state.orderBody).toBeUndefined();
  expect(state.orderAuthorization).toBeUndefined();
});

test("deep-link query string is preserved verbatim through the intercept", async ({
  page,
}) => {
  // The plan §5 contract says `return_to` carries the full path including
  // query. Use a URL-encoded amalgam of params that, if naively unescaped
  // or trimmed, would break.
  const target = `/markets/${JOURNEY_MARKET_ID}?side=yes&qty=10&ref=abc%20def&utm=spec-023`;
  const encoded = encodeURIComponent(target);

  await page.context().clearCookies();
  await page.goto(target, { waitUntil: "domcontentloaded" });
  await submitTrade(page);

  await expect(page).toHaveURL(
    new RegExp(`/auth/sign-in\\?return_to=${encoded}$`),
  );
});
