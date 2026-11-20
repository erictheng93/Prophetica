// Shared mock harness for SPEC-023 journey E2E specs.
//
// Each spec file owns its own state (kycTier, balance, ...) and its own
// `setupJourneyMocks(state)` lifecycle, but the route handlers, payload
// builders, and submit helpers live here so each spec stays focused on
// assertions.

import { expect, type Page } from "@playwright/test";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

export const JOURNEY_PORTS = {
  userService: 43_284,
  apiGateway: 43_280,
  kycService: 43_285,
} as const;

export const JOURNEY_USER_ID = "E2E-USER";
export const JOURNEY_MARKET_ID = "e2e-market";

export const JOURNEY_USER_SERVICE_SESSION = "journey-user-service-session";

export type JourneyKycTier = "TIER_0" | "TIER_1" | "TIER_2" | "TIER_3";

/// Mutable state each spec maintains. The mock route handlers read from
/// this object on every request, so tests can flip values mid-scenario.
export interface JourneyMockState {
  kycTier: JourneyKycTier;
  /// Available wallet balance in **minor units** (1 = 0.01 銅幣).
  /// Wire format inverts the sign per the SDK convention.
  balanceMinor: number;
  /// Last order body received at POST /v1/markets/:id/orders.
  orderBody: unknown;
  /// Authorization header captured from the order POST.
  orderAuthorization: string | undefined;
  selfExclusionActive?: boolean;
  selfExclusionRecord?: Record<string, unknown> | null;
  redemptionOrders?: Array<Record<string, unknown>>;
  redemptionEntitlements?: Array<Record<string, unknown>>;
}

export function freshJourneyState(): JourneyMockState {
  return {
    kycTier: "TIER_0",
    balanceMinor: 0,
    orderBody: undefined,
    orderAuthorization: undefined,
    selfExclusionActive: false,
    selfExclusionRecord: null,
    redemptionOrders: [],
    redemptionEntitlements: [],
  };
}

export interface JourneyMocks {
  userService: ReturnType<typeof createServer>;
  apiGateway: ReturnType<typeof createServer>;
  kycService: ReturnType<typeof createServer>;
}

export async function setupJourneyMocks(
  state: JourneyMockState,
): Promise<JourneyMocks> {
  const userService = await createJsonServer(
    JOURNEY_PORTS.userService,
    async (request, response) => {
      if (request.method === "POST" && request.url === "/v1/auth/login") {
        writeJson(response, 200, {
          user_id: JOURNEY_USER_ID,
          session_token: JOURNEY_USER_SERVICE_SESSION,
          expires_at: "2030-01-01T00:00:00Z",
        });
        return;
      }
      writeJson(response, 404, { error: "not_found" });
    },
  );

  const apiGateway = await createJsonServer(
    JOURNEY_PORTS.apiGateway,
    async (request, response) => {
      if (
        request.method === "GET" &&
        request.url?.startsWith("/v1/markets?")
      ) {
        writeJson(response, 200, { markets: [marketPayload()] });
        return;
      }
      if (
        request.method === "GET" &&
        request.url === `/v1/markets/${JOURNEY_MARKET_ID}`
      ) {
        writeJson(response, 200, marketPayload());
        return;
      }
      if (
        request.method === "GET" &&
        request.url?.startsWith(
          `/internal/engine/markets/${JOURNEY_MARKET_ID}/book`,
        )
      ) {
        writeJson(response, 200, {
          market_id: JOURNEY_MARKET_ID,
          bids: [{ price_pts: 54, qty: 25 }],
          asks: [{ price_pts: 55, qty: 25 }],
        });
        return;
      }
      if (
        request.method === "GET" &&
        request.url === "/v1/me/wallet/balance"
      ) {
        writeJson(response, 200, {
          user_id: JOURNEY_USER_ID,
          available_minor_units: -state.balanceMinor,
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
        request.url === `/v1/markets/${JOURNEY_MARKET_ID}/orders`
      ) {
        state.orderAuthorization = request.headers.authorization;
        state.orderBody = await readJson(request);
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
      if (
        request.method === "GET" &&
        request.url === "/v1/responsible-gaming/self-exclusion"
      ) {
        writeJson(response, 200, {
          user_id: JOURNEY_USER_ID,
          active: Boolean(state.selfExclusionActive),
          record: state.selfExclusionRecord ?? null,
          source: "api-gateway",
        });
        return;
      }
      if (
        request.method === "POST" &&
        request.url === "/v1/responsible-gaming/self-exclusion"
      ) {
        const body = (await readJson(request)) as Record<string, unknown>;
        state.selfExclusionActive = true;
        state.selfExclusionRecord = {
          record_id: `rec-se-${Date.now()}`,
          user_id: JOURNEY_USER_ID,
          mode: body.mode,
          period: body.period ?? null,
          points_disposition: body.points_disposition ?? null,
          reason: body.reason ?? null,
          starts_at_epoch_seconds: Math.floor(Date.now() / 1000),
          ends_at_epoch_seconds:
            body.period === "six_months"
              ? Math.floor(Date.now() / 1000) + 180 * 86400
              : null,
          frozen_minor_units: -100000,
        };
        writeJson(response, 200, {
          record: state.selfExclusionRecord,
          ledger_idempotency_key: "ledger-se-001",
          replayed: false,
        });
        return;
      }
      if (
        request.method === "POST" &&
        request.url === "/v1/redemption/orders"
      ) {
        const body = (await readJson(request)) as Record<string, unknown>;
        const orderId = `ord-red-${Date.now()}`;
        const entitlementId = `ent-${Date.now()}`;
        const order = {
          order_id: orderId,
          state: "accepted",
          user_id: JOURNEY_USER_ID,
          sku_id: body.sku_id,
          price_minor_units: 50000,
          price_points: 500,
          entitlement_id: entitlementId,
          idempotency_key: body.idempotency_key ?? "idemp-001",
          replayed: false,
        };
        state.redemptionOrders = state.redemptionOrders ?? [];
        state.redemptionOrders.push(order);
        state.redemptionEntitlements = state.redemptionEntitlements ?? [];
        state.redemptionEntitlements.push({
          entitlement_id: entitlementId,
          order_id: orderId,
          owner_user_id: JOURNEY_USER_ID,
          sku_id: body.sku_id,
          state: "active",
          fulfillment_payload: { code: "REDEEM-PROMO-999" },
        });
        writeJson(response, 200, order);
        return;
      }
      if (
        request.method === "GET" &&
        request.url?.startsWith("/v1/redemption/entitlements/")
      ) {
        writeJson(response, 200, {
          entitlements: state.redemptionEntitlements ?? [],
        });
        return;
      }
      if (
        request.method === "GET" &&
        request.url?.startsWith("/v1/redemption/orders/")
      ) {
        const orderId = request.url.split("/").pop();
        const found = state.redemptionOrders?.find(
          (o) => o.order_id === orderId,
        );
        if (found) {
          writeJson(response, 200, found);
        } else {
          writeJson(response, 404, { error: "order_not_found" });
        }
        return;
      }
      writeJson(response, 404, { error: "not_found" });
    },
  );

  const kycService = await createJsonServer(
    JOURNEY_PORTS.kycService,
    async (request, response) => {
      if (
        request.method === "GET" &&
        request.url === `/api/v1/kyc/users/${JOURNEY_USER_ID}/status`
      ) {
        writeJson(response, 200, kycStatusPayload(state.kycTier));
        return;
      }
      if (
        request.method === "POST" &&
        request.url === `/api/v1/kyc/users/${JOURNEY_USER_ID}/tier-1`
      ) {
        await readJson(request);
        state.kycTier = "TIER_1";
        writeJson(response, 200, {
          user_id: JOURNEY_USER_ID,
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

  return { userService, apiGateway, kycService };
}

export async function closeJourneyMocks(mocks: JourneyMocks): Promise<void> {
  await Promise.all([
    closeServer(mocks.userService),
    closeServer(mocks.apiGateway),
    closeServer(mocks.kycService),
  ]);
}

function marketPayload() {
  return {
    id: JOURNEY_MARKET_ID,
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

function kycStatusPayload(tier: JourneyKycTier) {
  const passed = tier !== "TIER_0";
  return {
    user_id: JOURNEY_USER_ID,
    current_tier: tier,
    status: passed ? "passed" : "pending",
    provider: passed ? "mock" : null,
    provider_applicant_id: null,
    country: passed ? "TW" : null,
    tier1_record_id: passed ? "tier1_e2e" : null,
    declared_monthly_income: null,
    submitted_at: passed ? "2026-01-01T00:00:00Z" : null,
    reviewed_at: passed ? "2026-01-01T00:00:00Z" : null,
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

/// Anchors `path` (which may contain query string) as an exact-match
/// RegExp, escaping all regex metacharacters so `?`, `&` etc. are literal.
export function pathToRegExp(path: string): RegExp {
  return new RegExp(`${path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
}

/// Drives the visitor → trade → intercepted-to-sign-in → login flow, ending
/// back on `marketReturnPath`. Used by specs whose focus is what happens
/// AFTER login (e.g. single-gate isolation).
export async function signInFromMarketTrade(
  page: Page,
  marketReturnPath: string,
): Promise<void> {
  const encoded = encodeURIComponent(marketReturnPath);
  await page.context().clearCookies();
  await page.goto(marketReturnPath, { waitUntil: "domcontentloaded" });
  await submitTrade(page);
  await expect(page).toHaveURL(
    new RegExp(`/auth/sign-in\\?return_to=${encoded}`),
  );
  await page.locator('input[name="identifier"]').fill("journey@example.com");
  await page
    .locator('input[name="password"]')
    .fill("correct horse battery staple");
  await page.getByRole("button", { name: "登入" }).click();
  await expect(page).toHaveURL(pathToRegExp(marketReturnPath));
}

/// Fills and submits the Tier 1 KYC form (the mock kyc-service flips
/// state.kycTier to TIER_1 on receipt).
export async function completeTierOne(page: Page): Promise<void> {
  await page.locator('input[name="legalName"]').fill("Journey User");
  await page.locator('input[name="birthday"]').fill("1990-01-02");
  await page.locator('select[name="country"]').selectOption("TW");
  await page.locator('input[name="documentId"]').fill("A123456789");
  await page.locator('input[name="consent"]').check();
  await page.getByRole("button", { name: "送出基礎驗證" }).click();
}

export async function submitTrade(page: Page) {
  const panel = page.locator("aside");
  await expect(panel).toHaveAttribute("data-hydrated", "true");
  await panel.locator('input[name="side"][value="YES"]').check({ force: true });
  await panel.locator('input[name="priceCents"]').fill("55");
  await panel.locator('input[name="quantity"]').fill("10");
  const submitButton = panel.getByRole("button", { name: "送出訂單" });
  await expect(submitButton).toBeEnabled();
  await submitButton.scrollIntoViewIfNeeded();
  await submitButton.evaluate((button) => {
    (button as HTMLButtonElement).click();
  });
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
