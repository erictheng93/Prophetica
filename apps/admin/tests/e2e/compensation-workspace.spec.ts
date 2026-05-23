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

test("manager proposal can be approved by admin and moves to posted history", async ({
  context,
  page,
}) => {
  const state = createCompensationState();
  const gateway = createMockGateway(state);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  try {
    await setViewer(context, "U-MANAGER-001", "manager");
    await page.goto("/compensation");
    await expect(page.getByRole("heading", { name: "補償提案" })).toBeVisible();

    await page.getByLabel("subject_user_id").fill("MEM-COMP-001");
    await page.getByLabel("ticket").fill("OPS-COMP-001");
    await page.getByLabel("amount").fill("125.00");
    await page.getByLabel("reason").fill("Playwright service credit");
    await page.getByRole("button", { name: "建立提案" }).click();
    await expect(page.getByText(/已建立補償提案 cmp-000001/)).toBeVisible();

    await setViewer(context, "U-ADMIN-001", "admin");
    await page.reload();
    await expect(page.getByText("cmp-000001")).toBeVisible();
    await page.getByRole("button", { name: "approve" }).click();
    await expect(page.getByText(/已核准 cmp-000001，目前狀態：已入帳/)).toBeVisible();
    await expect(page.getByText("已入帳")).toBeVisible();
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

test("manager sees own proposal approval disabled and backend rejects self review", async ({
  context,
  page,
}) => {
  const state = createCompensationState();
  const gateway = createMockGateway(state);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  try {
    await setViewer(context, "U-MANAGER-002", "manager");
    await page.goto("/compensation");
    await page.getByLabel("subject_user_id").fill("MEM-COMP-SELF");
    await page.getByLabel("ticket").fill("OPS-COMP-SELF");
    await page.getByLabel("amount").fill("50.00");
    await page.getByLabel("reason").fill("Self approval guard");
    await page.getByRole("button", { name: "建立提案" }).click();
    await expect(page.getByText("不可審自己的提案")).toBeVisible();
    await expect(page.getByRole("button", { name: "approve" })).toBeDisabled();

    const response = await page.request.post(
      "/api/compensation/cmp-000001/review",
      { data: { decision: "approved" } },
    );
    expect(response.status()).toBe(403);
  } finally {
    await new Promise<void>((resolve) => gateway.close(() => resolve()));
  }
});

test("posting_unknown is visually distinct and admin retry reconciles to posted", async ({
  context,
  page,
}) => {
  const state = createCompensationState({ nextApprovalOutcome: "posting_unknown" });
  const gateway = createMockGateway(state);
  await new Promise<void>((resolve) => gateway.listen(32123, resolve));

  try {
    await setViewer(context, "U-MANAGER-003", "manager");
    await page.goto("/compensation");
    await page.getByLabel("subject_user_id").fill("MEM-COMP-UNKNOWN");
    await page.getByLabel("ticket").fill("OPS-COMP-UNKNOWN");
    await page.getByLabel("amount").fill("75.00");
    await page.getByLabel("reason").fill("Ledger timeout path");
    await page.getByRole("button", { name: "建立提案" }).click();

    await setViewer(context, "U-ADMIN-003", "admin");
    await page.reload();
    await page.getByRole("button", { name: "approve" }).click();
    await expect(page.getByText(/結果待確認（可能已入帳）/)).toBeVisible();
    await expect(page.getByText("timeout: ledger request timed out")).toBeVisible();

    await page.getByRole("button", { name: "retry posting" }).click();
    await expect(page.getByText(/已 reconcile cmp-000001，未重複入帳/)).toBeVisible();
    await expect(page.getByText("已入帳")).toBeVisible();
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

interface Proposal {
  proposal_id: string;
  user_id: string;
  ticket_id: string;
  reason: string;
  amount_minor_units: number;
  status:
    | "proposed"
    | "posting"
    | "posted"
    | "posting_failed"
    | "posting_unknown"
    | "rejected";
  proposer_kind: "manager" | "admin";
  proposer_user_id: string;
  approvals: Array<{ approver_user_id: string; role: string }>;
  ledger_transaction_posted: boolean;
  ledger_idempotency_key: string | null;
  ledger_transaction_id: number | null;
  posting_attempt_count: number;
  last_posting_error: string | null;
  approved_at: string | null;
}

interface CompensationState {
  proposals: Proposal[];
  nextId: number;
  nextApprovalOutcome: "posted" | "posting_unknown";
}

function createCompensationState(
  input: Partial<Pick<CompensationState, "nextApprovalOutcome">> = {},
): CompensationState {
  return {
    proposals: [],
    nextId: 1,
    nextApprovalOutcome: input.nextApprovalOutcome ?? "posted",
  };
}

function createMockGateway(state: CompensationState) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1:32123");
    const method = request.method ?? "GET";
    const actorId = request.headers["x-actor-id"]?.toString() ?? "unknown";
    const role = request.headers["x-operator-role"]?.toString() as
      | "admin"
      | "manager"
      | undefined;

    if (
      method === "GET" &&
      url.pathname === "/api/internal/admin/compensation-proposals"
    ) {
      const status = url.searchParams.get("status");
      sendJson(response, {
        proposals: state.proposals.filter((proposal) => proposal.status === status),
        page: 1,
        page_size: 25,
        total: state.proposals.filter((proposal) => proposal.status === status).length,
        source: "playwright:compensation",
      });
      return;
    }

    const createMatch = /^\/api\/internal\/admin\/members\/([^/]+)\/compensation-proposals$/.exec(
      url.pathname,
    );
    if (method === "POST" && createMatch) {
      const body = (await readJson(request)) as {
        ticket_id: string;
        reason: string;
        amount_minor_units: number;
      };
      const proposal: Proposal = {
        proposal_id: `cmp-${String(state.nextId).padStart(6, "0")}`,
        user_id: decodeURIComponent(createMatch[1] ?? ""),
        ticket_id: body.ticket_id,
        reason: body.reason,
        amount_minor_units: body.amount_minor_units,
        status: "proposed",
        proposer_kind: role === "admin" ? "admin" : "manager",
        proposer_user_id: actorId,
        approvals: [],
        ledger_transaction_posted: false,
        ledger_idempotency_key: null,
        ledger_transaction_id: null,
        posting_attempt_count: 0,
        last_posting_error: null,
        approved_at: null,
      };
      state.nextId += 1;
      state.proposals.unshift(proposal);
      sendJson(response, {
        proposal,
        audit: {
          action: "compensation_proposal_created",
          subject_user_id: proposal.user_id,
          ticket_id: proposal.ticket_id,
          correlation_id: `pw-${proposal.proposal_id}`,
          approval_roles: [],
          proposer_kind: proposal.proposer_kind,
          ledger_transaction_posted: false,
        },
      });
      return;
    }

    const reviewMatch = /^\/api\/internal\/admin\/compensation-proposals\/([^/]+)\/review$/.exec(
      url.pathname,
    );
    if (method === "POST" && reviewMatch) {
      const proposal = state.proposals.find(
        (item) => item.proposal_id === decodeURIComponent(reviewMatch[1] ?? ""),
      );
      if (!proposal) {
        sendJson(response, { error: "ERROR_CODE_COMPENSATION_PROPOSAL_NOT_FOUND" }, 404);
        return;
      }
      if (proposal.proposer_user_id === actorId) {
        sendJson(response, { error: "ERROR_CODE_COMPENSATION_SELF_REVIEW" }, 403);
        return;
      }
      const body = (await readJson(request)) as { decision: "approved" | "rejected" };
      if (body.decision === "rejected") {
        proposal.status = "rejected";
      } else if (state.nextApprovalOutcome === "posting_unknown") {
        proposal.status = "posting_unknown";
        proposal.ledger_idempotency_key = `cs-cmp-${proposal.proposal_id}`;
        proposal.posting_attempt_count = 1;
        proposal.last_posting_error = "timeout: ledger request timed out";
        proposal.approved_at = "2026-05-23T12:00:00Z";
      } else {
        proposal.status = "posted";
        proposal.ledger_transaction_posted = true;
        proposal.ledger_transaction_id = 9001;
        proposal.ledger_idempotency_key = `cs-cmp-${proposal.proposal_id}`;
        proposal.posting_attempt_count = 1;
        proposal.approved_at = "2026-05-23T12:00:00Z";
      }
      proposal.approvals = [{ approver_user_id: actorId, role: role ?? "admin" }];
      sendJson(response, {
        proposal,
        review: {
          reviewer_user_id: actorId,
          reviewer_role: role ?? "admin",
          decision: body.decision,
          note: null,
        },
        replayed: false,
      });
      return;
    }

    const retryMatch = /^\/api\/internal\/admin\/compensation-proposals\/([^/]+)\/retry-posting$/.exec(
      url.pathname,
    );
    if (method === "POST" && retryMatch) {
      const proposal = state.proposals.find(
        (item) => item.proposal_id === decodeURIComponent(retryMatch[1] ?? ""),
      );
      if (!proposal) {
        sendJson(response, { error: "ERROR_CODE_COMPENSATION_PROPOSAL_NOT_FOUND" }, 404);
        return;
      }
      proposal.status = "posted";
      proposal.ledger_transaction_posted = true;
      proposal.ledger_transaction_id = 9901;
      proposal.last_posting_error = null;
      sendJson(response, { proposal, replayed: true });
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
