import { expect, test } from "@playwright/test";

import {
  closeJourneyMocks,
  freshJourneyState,
  setupJourneyMocks,
  type JourneyMockState,
  type JourneyMocks,
} from "./_helpers/journey-mocks";

// SPEC-023 plan Task 4.7 / INV-JOURNEY-4 — round-trip open-redirect
// regression coverage. The unit test in apps/web/tests/safe-return-to.test.ts
// already pins each rejection class for the helper itself; this spec
// confirms the rejection survives the full form submit + server action
// + redirect roundtrip (i.e. nothing downstream of safeReturnTo passes
// the raw value through).

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

const REJECTION_CASES: Array<{ name: string; rawReturnTo: string }> = [
  {
    name: "protocol-relative URL",
    rawReturnTo: "//evil.example/x",
  },
  {
    name: "javascript: scheme",
    rawReturnTo: "javascript:alert(1)",
  },
  {
    name: "encoded CRLF injection",
    rawReturnTo: "/markets/abc%0d%0aLocation:%20/x",
  },
  {
    name: "encoded cross-domain absolute URL",
    rawReturnTo: "https%3A%2F%2Fevil.example%2Fx",
  },
];

for (const { name, rawReturnTo } of REJECTION_CASES) {
  test(`sign-in falls back to /portfolio for ${name}`, async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(
      `/auth/sign-in?return_to=${encodeURIComponent(rawReturnTo)}`,
      { waitUntil: "domcontentloaded" },
    );
    await page.locator('input[name="identifier"]').fill("journey@example.com");
    await page
      .locator('input[name="password"]')
      .fill("correct horse battery staple");
    await page.getByRole("button", { name: "登入" }).click();

    await expect(page).toHaveURL(new RegExp("/portfolio$"));
  });
}
