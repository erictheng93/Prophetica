import { expect, test, type BrowserContext } from "@playwright/test";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { ADMIN_ORIGIN } from "../admin-env.js";
import { signSession } from "../../lib/session-crypto.js";

const SESSION_COOKIE_NAME = "proph_admin_session";
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";

test("admin approves commission adjustment through referral-service review queue", async ({
  context,
  page,
}) => {
  const state = createAffiliateReviewState();
  const gateway = createMockGateway(state);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  try {
    await setViewer(context, "U-ADMIN-REVIEW", "admin");
    await page.goto("/affiliate/commissions");

    await expect(page.getByRole("heading", { name: "佣金調整佇列" })).toBeVisible();
    await expect(page.getByText("COM-REVIEW-001")).toBeVisible();
    await expect(page.getByText("pending")).toBeVisible();

    const row = page.getByRole("row").filter({ hasText: "COM-REVIEW-001" });
    await row.getByPlaceholder("review reason").fill("invoice reconciled");
    await row.getByRole("button", { name: "approve" }).click();

    await expect(row.getByText("approved")).toBeVisible();
    await expect(row.getByText("已完成審核")).toBeVisible();
    await expect(row.getByRole("button", { name: "approve" })).toHaveCount(0);
    await expect(row.getByRole("button", { name: "reject" })).toHaveCount(0);
    expect(state.decisions).toContainEqual({
      path: "/api/internal/admin/affiliate-commission-adjustments/COM-REVIEW-001/review",
      actorId: "U-ADMIN-REVIEW",
      role: "admin",
      body: {
        decision: "approved",
        reason: "invoice reconciled",
      },
    });
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

test("manager rejects payout review through referral-service review queue", async ({
  context,
  page,
}) => {
  const state = createAffiliateReviewState();
  const gateway = createMockGateway(state);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  try {
    await setViewer(context, "U-MANAGER-REVIEW", "manager");
    await page.goto("/affiliate/payouts");

    await expect(page.getByRole("heading", { name: "撥款申請佇列" })).toBeVisible();
    await expect(page.getByText("PAY-REVIEW-001")).toBeVisible();
    await expect(page.getByText("pending")).toBeVisible();

    const row = page.getByRole("row").filter({ hasText: "PAY-REVIEW-001" });
    await row.getByPlaceholder("review reason").fill("bank verification failed");
    await row.getByRole("button", { name: "reject" }).click();

    await expect(row.getByText("rejected")).toBeVisible();
    await expect(row.getByText("已完成審核")).toBeVisible();
    await expect(row.getByRole("button", { name: "approve" })).toHaveCount(0);
    await expect(row.getByRole("button", { name: "reject" })).toHaveCount(0);
    expect(state.decisions).toContainEqual({
      path: "/api/internal/admin/affiliate-payouts/PAY-REVIEW-001/review",
      actorId: "U-MANAGER-REVIEW",
      role: "manager",
      body: {
        decision: "rejected",
        reason: "bank verification failed",
      },
    });
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

async function setViewer(
  context: BrowserContext,
  sub: string,
  role: "admin" | "manager",
) {
  await context.addCookies([
    {
      name: SESSION_COOKIE_NAME,
      value: await signSession({ sub, role }, DEV_SECRET),
      url: ADMIN_ORIGIN,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);
}

interface ReviewDecision {
  path: string;
  actorId: string;
  role: string;
  body: {
    decision: "approved" | "rejected";
    reason: string;
  };
}

interface AffiliateReviewState {
  commissionAdjustments: Array<{
    id: string;
    affiliateId: string;
    adjustmentType: "credit" | "debit";
    amountMinor: number;
    currency: "USDT";
    reason: string;
    status: string;
    requestedBy: string;
    requestedAt: string;
  }>;
  payouts: Array<{
    id: string;
    affiliateId: string;
    amountMinor: number;
    currency: "USDT";
    requestedAt: string;
    status: string;
  }>;
  decisions: ReviewDecision[];
}

function createAffiliateReviewState(): AffiliateReviewState {
  return {
    commissionAdjustments: [
      {
        id: "COM-REVIEW-001",
        affiliateId: "AFF-REVIEW",
        adjustmentType: "credit",
        amountMinor: 42_000,
        currency: "USDT",
        reason: "manual reconciliation",
        status: "pending",
        requestedBy: "ops",
        requestedAt: "2026-06-29 10:00",
      },
    ],
    payouts: [
      {
        id: "PAY-REVIEW-001",
        affiliateId: "AFF-REVIEW",
        amountMinor: 120_000,
        currency: "USDT",
        requestedAt: "2026-06-29 10:10",
        status: "pending",
      },
    ],
    decisions: [],
  };
}

function createMockGateway(state: AffiliateReviewState) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1:32123");
    const method = request.method ?? "GET";
    const actorId = request.headers["x-actor-id"]?.toString() ?? "unknown";
    const role = request.headers["x-operator-role"]?.toString() ?? "unknown";

    if (
      method === "GET" &&
      url.pathname === "/api/internal/admin/affiliate-commission-adjustments"
    ) {
      sendJson(response, state.commissionAdjustments);
      return;
    }

    if (method === "GET" && url.pathname === "/api/internal/admin/affiliate-payouts") {
      sendJson(response, state.payouts);
      return;
    }

    const commissionReviewMatch =
      /^\/api\/internal\/admin\/affiliate-commission-adjustments\/([^/]+)\/review$/.exec(
        url.pathname,
      );
    if (method === "POST" && commissionReviewMatch) {
      const reviewId = decodeURIComponent(commissionReviewMatch[1] ?? "");
      const item = state.commissionAdjustments.find((review) => review.id === reviewId);
      if (!item) {
        sendJson(response, { error: "not_found" }, 404);
        return;
      }
      const body = (await readJson(request)) as ReviewDecision["body"];
      item.status = body.decision;
      state.decisions.push({ path: url.pathname, actorId, role, body });
      sendJson(response, {
        review: item,
        auditCorrelationId: `pw-affiliate-review:${reviewId}`,
      });
      return;
    }

    const payoutReviewMatch =
      /^\/api\/internal\/admin\/affiliate-payouts\/([^/]+)\/review$/.exec(
        url.pathname,
      );
    if (method === "POST" && payoutReviewMatch) {
      const reviewId = decodeURIComponent(payoutReviewMatch[1] ?? "");
      const item = state.payouts.find((review) => review.id === reviewId);
      if (!item) {
        sendJson(response, { error: "not_found" }, 404);
        return;
      }
      const body = (await readJson(request)) as ReviewDecision["body"];
      item.status = body.decision;
      state.decisions.push({ path: url.pathname, actorId, role, body });
      sendJson(response, {
        review: item,
        auditCorrelationId: `pw-affiliate-review:${reviewId}`,
      });
      return;
    }

    sendJson(response, { error: "not_found" }, 404);
  });
}

function readJson(request: IncomingMessage) {
  return new Promise<unknown>((resolve, reject) => {
    const chunks: Buffer[] = [];
    request.on("data", (chunk: Buffer) => chunks.push(chunk));
    request.on("end", () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}"));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

function sendJson(response: ServerResponse, body: unknown, status = 200) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}
