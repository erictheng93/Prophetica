import { expect, test } from "@playwright/test";
import { createServer, type ServerResponse } from "node:http";
import { ADMIN_ORIGIN } from "../admin-env.js";
import { signSession } from "../../lib/session-crypto.js";

const SESSION_COOKIE_NAME = "proph_admin_session";
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";

const memberSummary = {
  member: {
    user_id: "MEM-1042",
    display_name: "陳柏安",
    status: "restricted",
    tier: "Standard",
    registered_at: "2026-04-02T16:05:00+08:00",
    contact: {
      email_masked: "c***@example.net",
      phone_masked: "0987***112",
      line_id_masked: null,
      country: "TW",
      timezone: "Asia/Taipei",
    },
  },
  account: {
    available_minor_units: -54000,
    locked_minor_units: -120000,
    frozen_minor_units: -18000,
    currency: "POINTS",
    last_login_at: "2026-05-14T08:10:00+08:00",
    device_count: 5,
    two_factor_enabled: false,
  },
  kyc_risk: {
    kyc_status: "review_required",
    risk_level: "high",
    risk_score: 86,
    last_reviewed_at: "2026-05-14T08:45:00+08:00",
    restrictions: ["manual_review_required"],
  },
  open_cases: [],
  audit: {
    correlation_id: "admin-member-MEM-1042",
    source: "playwright:member-management",
    audit_required: true,
    latest_admin_action_at: null,
  },
  source: {
    channel: "affiliate",
    affiliate_id: "AFF-014",
    affiliate_name: "AFF-014 / 社群代理",
  },
};

test("admin can submit and approve a member restriction action", async ({
  context,
  page,
}) => {
  const actions: unknown[] = [];
  const gatewayRequests: string[] = [];
  const gateway = createMockGateway(actions, gatewayRequests);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));
  await expect(
    fetch("http://127.0.0.1:32123/api/internal/admin/members/MEM-1042/summary"),
  ).resolves.toMatchObject({ ok: true });

  const consoleErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => consoleErrors.push(error.message));

  await context.addCookies([
    {
      name: SESSION_COOKIE_NAME,
      value: await signSession(
        {
          sub: "U-ADMIN-001",
          role: "admin",
        },
        DEV_SECRET,
      ),
      url: ADMIN_ORIGIN,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);

  try {
    const summaryResponse = await page.request.get(
      "/api/members/MEM-1042/summary",
    );
    expect(summaryResponse.ok()).toBe(true);
    await expect(summaryResponse).toBeOK();
    expect(await summaryResponse.json()).toMatchObject({
      member: { user_id: "MEM-1042" },
      audit: { source: "playwright:member-management" },
    });

    const pageSummaryResponsePromise = page.waitForResponse((response) =>
      response.url().includes("/api/members/MEM-1042/summary"),
    );
    await page.goto("/members?query=MEM-1042");
    expect((await pageSummaryResponsePromise).ok()).toBe(true);

    await expect(page).toHaveTitle(/Prophetica Admin/);
    await expect(
      page.getByRole("heading", { name: "會員待處理 API" }),
    ).toBeHidden();
    await expect(
      page.getByRole("heading", { name: "工單審批", exact: true }),
    ).toBeHidden();
    await expect(
      page.getByRole("heading", { name: "提交處理動作" }),
    ).toBeVisible();

    const actionPanel = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "提交處理動作" }) });
    await actionPanel.getByLabel("動作").selectOption("restrict_withdrawals");
    await actionPanel.getByLabel("工單 ID").fill("OPS-PW-1042");
    await actionPanel.getByLabel("處理原因").fill("Playwright risk hold");
    await actionPanel.getByRole("button", { name: "提交動作" }).click();

    await expect(page.getByText(/已提交 mem-act-000001/)).toBeVisible();

    await page.goto("/approvals");
    await expect(
      page.getByText(/提交提款限制 \/ MEM-1042 \/ OPS-PW-1042/),
    ).toBeVisible();

    const approvalPanel = page.locator("section").filter({
      has: page.getByRole("heading", { name: "工單審批", exact: true }),
    });
    await approvalPanel
      .getByPlaceholder("審批原因")
      .fill("Playwright second approval");
    const reviewResponsePromise = page.waitForResponse((response) =>
      response.url().includes("/api/members/actions/mem-act-000001/reviews"),
    );
    await approvalPanel.getByRole("button", { name: "核准" }).click();
    expect((await reviewResponsePromise).ok()).toBe(true);

    await expect(page.getByText("approved")).toBeVisible();
    await expect(
      page.getByText(/effects: member_restriction:withdrawals_locked/),
    ).toBeVisible();
    expect(gatewayRequests).toContain(
      "POST /api/internal/admin/members/actions/mem-act-000001/reviews",
    );
    expect(consoleErrors).toEqual([]);
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

test("member management sidebar section stays open when navigating to KYC review", async ({
  context,
  page,
}) => {
  const gateway = createMockGateway([], []);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  await context.addCookies([
    {
      name: SESSION_COOKIE_NAME,
      value: await signSession(
        {
          sub: "U-ADMIN-001",
          role: "admin",
        },
        DEV_SECRET,
      ),
      url: ADMIN_ORIGIN,
      httpOnly: true,
      sameSite: "Strict",
    },
  ]);

  try {
    await page.goto("/members");
    await expect(page.getByRole("heading", { name: "會員管理" })).toBeVisible();

    const nav = page.getByRole("navigation", { name: "管理後台導覽" });
    const memberSection = nav.getByRole("button", { name: "會員管理" });
    await memberSection.click();
    await expect(memberSection).toHaveAttribute("aria-expanded", "true");

    await nav.getByRole("link", { name: "KYC 審核" }).click();
    await expect(page).toHaveURL(/\/members\/kyc-review$/);
    await expect(page.getByRole("heading", { name: "KYC 審核" })).toBeVisible();
    await expect(memberSection).toHaveAttribute("aria-expanded", "true");
    await expect(nav.getByRole("link", { name: "KYC 審核" })).toBeVisible();
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

function createMockGateway(actions: unknown[], gatewayRequests: string[]) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1:32123");
    const method = request.method ?? "GET";
    gatewayRequests.push(`${method} ${url.pathname}`);

    if (
      method === "GET" &&
      url.pathname === "/api/internal/admin/members/pending-cases"
    ) {
      sendJson(response, {
        items: [
          {
            member_id: "MEM-1042",
            display_name: "陳柏安",
            restriction_reason: "manual_review_required",
            contact: {
              email_masked: "c***@example.net",
              phone_masked: "0987***112",
            },
            status: "restricted",
            kyc_status: "review_required",
            affiliate_id: "AFF-014",
          },
        ],
        page: 1,
        page_size: 5,
        total: 1,
        source: "api-gateway",
      });
      return;
    }

    if (method === "GET" && url.pathname === "/api/internal/admin/members") {
      sendJson(response, {
        members: [memberSummary],
        source: "user-service:member-store",
      });
      return;
    }

    if (
      method === "GET" &&
      url.pathname === "/api/internal/admin/members/MEM-1042/summary"
    ) {
      sendJson(response, memberSummary);
      return;
    }

    if (
      method === "GET" &&
      url.pathname === "/api/internal/admin/members/actions"
    ) {
      sendJson(response, {
        actions,
        source: "api-gateway",
      });
      return;
    }

    if (
      method === "POST" &&
      url.pathname === "/api/internal/admin/members/actions"
    ) {
      const action = {
        action_id: "mem-act-000001",
        target_user_id: "MEM-1042",
        action_type: "restrict_withdrawals",
        ticket_id: "OPS-PW-1042",
        reason: "Playwright risk hold",
        status: "pending_second_approval",
        requires_second_approval: true,
        actor_user_id: "U-ADMIN-001",
        actor_role: "admin",
        correlation_id: "playwright-member-action",
        review: null,
        side_effects: [],
        created_at: "2026-05-16T00:00:00Z",
      };
      actions[0] = action;
      sendJson(
        response,
        {
          action_id: action.action_id,
          target_user_id: action.target_user_id,
          action_type: action.action_type,
          status: action.status,
          requires_second_approval: true,
          audit: {
            actor_user_id: "U-ADMIN-001",
            actor_role: "admin",
            target_user_id: action.target_user_id,
            ticket_id: action.ticket_id,
            correlation_id: action.correlation_id,
            ledger_transaction_posted: false,
          },
        },
        202,
      );
      return;
    }

    if (
      method === "POST" &&
      url.pathname ===
        "/api/internal/admin/members/actions/mem-act-000001/reviews"
    ) {
      const action = {
        ...(actions[0] as Record<string, unknown>),
        status: "approved",
        review: {
          reviewer_user_id: "U-ADMIN-001",
          reviewer_role: "admin",
          decision: "approve",
          reason: "Playwright second approval",
          reviewed_at: "2026-05-16T00:00:00Z",
        },
        side_effects: ["member_restriction:withdrawals_locked"],
      };
      actions[0] = action;
      sendJson(response, action);
      return;
    }

    if (
      method === "GET" &&
      url.pathname.startsWith("/api/internal/admin/members/") &&
      url.pathname.endsWith("/kyc-documents")
    ) {
      // KYC document chain (docs/implementation-plans/2026-05-19-kyc-document-
      // review-chain-plan.md): KycDocumentGallery fetches this for every member
      // summary. The restriction-action tests do not exercise documents, so an
      // empty list lets the gallery render its "尚無文件。" state with no
      // follow-up <img> requests — and no console 404s.
      sendJson(response, []);
      return;
    }

    sendJson(response, { error: "not_found" }, 404);
  });
}

function sendJson(response: ServerResponse, body: unknown, status = 200) {
  response.writeHead(status, {
    "content-type": "application/json",
  });
  response.end(JSON.stringify(body));
}
