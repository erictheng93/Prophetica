import { expect, test, type Page } from "@playwright/test";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

const USER_SERVICE_PORT = 43_284;
const API_GATEWAY_PORT = 43_280;
const KYC_SERVICE_PORT = 43_285;
const USER_ID = "E2E-USER";
const MARKET_ID = "e2e-market";

let kycTier: "TIER_0" | "TIER_1" = "TIER_0";
let balanceMinor = 0;
let orderBody: unknown;
let orderAuthorization: string | undefined;
let userService: ReturnType<typeof createServer>;
let apiGateway: ReturnType<typeof createServer>;
let kycService: ReturnType<typeof createServer>;

test.beforeAll(async () => {
  userService = await createJsonServer(
    USER_SERVICE_PORT,
    async (request, response) => {
      if (request.method === "POST" && request.url === "/v1/auth/login") {
        writeJson(response, 200, {
          user_id: USER_ID,
          session_token: "journey-user-service-session",
          expires_at: "2030-01-01T00:00:00Z",
        });
        return;
      }
      writeJson(response, 404, { error: "not_found" });
    },
  );

  apiGateway = await createJsonServer(
    API_GATEWAY_PORT,
    async (request, response) => {
      if (request.method === "GET" && request.url?.startsWith("/v1/markets?")) {
        writeJson(response, 200, { markets: [marketPayload()] });
        return;
      }
      if (
        request.method === "GET" &&
        request.url === `/v1/markets/${MARKET_ID}`
      ) {
        writeJson(response, 200, marketPayload());
        return;
      }
      if (
        request.method === "GET" &&
        request.url?.startsWith(`/internal/engine/markets/${MARKET_ID}/book`)
      ) {
        writeJson(response, 200, {
          market_id: MARKET_ID,
          bids: [{ price_pts: 54, qty: 25 }],
          asks: [{ price_pts: 55, qty: 25 }],
        });
        return;
      }
      if (request.method === "GET" && request.url === "/v1/me/wallet/balance") {
        writeJson(response, 200, {
          user_id: USER_ID,
          available_minor_units: -balanceMinor,
          locked_minor_units: 0,
          frozen_minor_units: 0,
        });
        return;
      }
      if (
        request.method === "GET" &&
        request.url?.startsWith("/v1/ledger/entries")
      ) {
        writeJson(response, 200, { entries: [] });
        return;
      }
      if (
        request.method === "GET" &&
        request.url === "/v1/payments/real-money-status"
      ) {
        writeJson(response, 200, { real_money_enabled: false });
        return;
      }
      if (
        request.method === "POST" &&
        request.url === `/v1/markets/${MARKET_ID}/orders`
      ) {
        orderAuthorization = request.headers.authorization;
        orderBody = await readJson(request);
        writeJson(response, 200, {
          order_id: "ord-journey-1",
          status: "accepted",
          filled_qty: 0,
          avg_price_pts: 0,
          remaining_qty: 10,
          fee_pts: 0,
        });
        return;
      }
      writeJson(response, 404, { error: "not_found" });
    },
  );

  kycService = await createJsonServer(
    KYC_SERVICE_PORT,
    async (request, response) => {
      if (
        request.method === "GET" &&
        request.url === `/api/v1/kyc/users/${USER_ID}/status`
      ) {
        writeJson(response, 200, kycStatusPayload());
        return;
      }
      if (
        request.method === "POST" &&
        request.url === `/api/v1/kyc/users/${USER_ID}/tier-1`
      ) {
        await readJson(request);
        kycTier = "TIER_1";
        writeJson(response, 200, {
          user_id: USER_ID,
          target_tier: "TIER_1",
          status: "passed",
          provider: "mock",
          document_id_hash: "doc_hash",
          replayed: false,
        });
        return;
      }
      writeJson(response, 404, { error: "not_found" });
    },
  );
});

test.beforeEach(() => {
  kycTier = "TIER_0";
  balanceMinor = 0;
  orderBody = undefined;
  orderAuthorization = undefined;
});

test.afterAll(async () => {
  await Promise.all([
    closeServer(userService),
    closeServer(apiGateway),
    closeServer(kycService),
  ]);
});

test("visitor can complete sign-in, KYC, top-up assistance, and order journey", async ({
  page,
}) => {
  const marketReturnPath = `/markets/${MARKET_ID}?side=yes&qty=10`;
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
    new RegExp(`/markets/${MARKET_ID}\\?side=yes&qty=10$`),
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
    new RegExp(`/markets/${MARKET_ID}\\?side=yes&qty=10$`),
  );

  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/topup\\?return_to=${encodedMarketReturnPath}`),
  );

  balanceMinor = 10_000;
  await page.goto(`/markets/${MARKET_ID}`, { waitUntil: "domcontentloaded" });
  await submitTrade(page);

  await expect
    .poll(() => orderBody)
    .toMatchObject({
      side: "buy",
      outcome: "YES",
      type: "LIMIT",
      price_pts: 55,
      quantity: 10,
      time_in_force: "GTC",
    });
  expect(orderAuthorization).toBe("Bearer journey-user-service-session");
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

function marketPayload() {
  return {
    id: MARKET_ID,
    title: "Journey E2E Market",
    brief: "End-to-end onboarding market",
    category: "sports",
    status: "open",
    yes_price_cents: 55,
    yes_bid_cents: 54,
    yes_ask_cents: 55,
    volume_24h_points: 1_000,
    volume_total_points: 10_000,
    opened_at: "2026-01-01T00:00:00Z",
    close_at: "2026-12-31T00:00:00Z",
    oracle_source: "journey-e2e",
    resolved_outcome: null,
    resolved_at: null,
  };
}

function kycStatusPayload() {
  return {
    user_id: USER_ID,
    current_tier: kycTier,
    status: kycTier === "TIER_1" ? "passed" : "pending",
    provider: kycTier === "TIER_1" ? "mock" : null,
    provider_applicant_id: null,
    country: kycTier === "TIER_1" ? "TW" : null,
    tier1_record_id: kycTier === "TIER_1" ? "tier1_e2e" : null,
    declared_monthly_income: null,
    submitted_at: kycTier === "TIER_1" ? "2026-01-01T00:00:00Z" : null,
    reviewed_at: kycTier === "TIER_1" ? "2026-01-01T00:00:00Z" : null,
    expires_at: null,
  };
}

async function createJsonServer(
  port: number,
  handler: (
    request: IncomingMessage,
    response: ServerResponse,
  ) => Promise<void>,
) {
  const server = createServer((request, response) => {
    handler(request, response).catch(() => {
      writeJson(response, 500, { error: "internal" });
    });
  });
  await new Promise<void>((resolveListen) => {
    server.listen(port, "127.0.0.1", resolveListen);
  });
  return server;
}

async function submitTrade(page: Page) {
  const panel = page.locator("aside");
  await expect(panel).toHaveAttribute("data-hydrated", "true");
  await panel.locator('input[name="side"][value="YES"]').check({ force: true });
  await panel.locator('input[name="priceCents"]').fill("55");
  await panel.locator('input[name="quantity"]').fill("10");
  const submitButton = panel.getByRole("button", { name: "送出訂單" });
  await expect(submitButton).toBeEnabled();
  await submitButton.scrollIntoViewIfNeeded();
  await submitButton.click({ force: true });
}

async function readJson(request: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

function writeJson(response: ServerResponse, status: number, body: unknown) {
  response.writeHead(status, {
    "content-type": "application/json",
  });
  response.end(JSON.stringify(body));
}

async function closeServer(
  server: ReturnType<typeof createServer> | undefined,
) {
  await new Promise<void>((resolveClose) => {
    server?.close(() => resolveClose());
  });
}
