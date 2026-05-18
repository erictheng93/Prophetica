import { expect, test, type APIResponse } from "@playwright/test";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ADMIN_ORIGIN } from "../admin-env.js";
import { signSession } from "../../lib/session-crypto.js";

const SESSION_COOKIE_NAME = "proph_admin_session";
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";
const DATABASE_URL =
  process.env.ADMIN_FULL_E2E_DATABASE_URL ??
  "postgres://proph:dev@127.0.0.1:5432/prophetica";
const API_GATEWAY_URL = "http://127.0.0.1:43180";
const KYC_SERVICE_URL = "http://127.0.0.1:43185";
const WALLET_LEDGER_URL = "http://127.0.0.1:43186";
const RISK_SERVICE_URL = "http://127.0.0.1:43191";

test("admin UI completes a real member KYC action through gateway services and Postgres", async ({
  context,
  page,
}) => {
  const schemas = uniqueSchemas();
  preparePostgresSchemas(schemas);
  const services = await startFullChainServices(schemas);
  const userId = `MEM-PW-${Date.now()}`;
  await seedWalletBalance(userId, 12_500);

  await context.addCookies([
    {
      name: SESSION_COOKIE_NAME,
      value: await signSession(
        { sub: "U-MANAGER-001", role: "manager" },
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

    await page.getByRole("button", { name: "新增會員" }).click();
    const editor = page
      .getByRole("dialog")
      .filter({ has: page.getByRole("heading", { name: "新增會員" }) });
    await editor.getByLabel("會員 ID").fill(userId);
    await editor.getByLabel("顯示名稱").fill("Full Chain Member");
    await editor
      .getByLabel("Email")
      .fill(`${userId.toLowerCase()}@example.com`);
    await editor.getByLabel("手機").fill("0912999888");
    await editor.getByRole("button", { name: "新增會員" }).click();
    await expect(
      page.getByText("會員已新增，帳戶狀態為未激活。"),
    ).toBeVisible();

    const actionPanel = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "提交處理動作" }) });
    await actionPanel.getByLabel("動作").selectOption("request_kyc_review");
    await actionPanel.getByLabel("工單 ID").fill("OPS-FULL-KYC");
    await actionPanel.getByLabel("處理原因").fill("Full chain KYC review");
    await actionPanel.getByRole("button", { name: "提交動作" }).click();
    await expect(page.getByText(/已提交 mem-act-/)).toBeVisible();

    await page.goto("/approvals");
    await expect(
      page.getByText(new RegExp(`提交 KYC 覆核 / ${userId} / OPS-FULL-KYC`)),
    ).toBeVisible();

    await context.addCookies([
      {
        name: SESSION_COOKIE_NAME,
        value: await signSession(
          { sub: "U-ADMIN-002", role: "admin" },
          DEV_SECRET,
        ),
        url: ADMIN_ORIGIN,
        httpOnly: true,
        sameSite: "Strict",
      },
    ]);

    const approvalPanel = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "工單審批", exact: true }) });
    await approvalPanel
      .getByPlaceholder("審批原因")
      .fill("Full chain approval");
    await approvalPanel.getByRole("button", { name: "核准" }).click();
    await expect(page.getByText("approved")).toBeVisible();
    await expect(
      page.getByText(/effects: kyc_review_task:requested/),
    ).toBeVisible();

    const summary = await json(
      await page.request.get(`/api/members/${userId}/summary`),
    );
    expect(summary.member.status).toBe("restricted");
    expect(summary.kyc_risk.kyc_status).toBe("review_required");
    expect(summary.kyc_risk.restrictions).toContain("manual_review_required");

    const kycStatusResponse = await fetch(
      `${KYC_SERVICE_URL}/api/v1/kyc/users/${encodeURIComponent(userId)}/status`,
      { headers: { "x-user-id": userId } },
    );
    expect(kycStatusResponse.ok).toBe(true);
    const kycStatus = await kycStatusResponse.json();
    expect(kycStatus.current_tier).toBe("TIER_2");
    expect(kycStatus.status).toBe("pending");

    await context.addCookies([
      {
        name: SESSION_COOKIE_NAME,
        value: await signSession(
          { sub: "U-MANAGER-003", role: "manager" },
          DEV_SECRET,
        ),
        url: ADMIN_ORIGIN,
        httpOnly: true,
        sameSite: "Strict",
      },
    ]);
    await actionPanel.getByLabel("動作").selectOption("restrict_withdrawals");
    await actionPanel.getByLabel("工單 ID").fill("OPS-FULL-WALLET");
    await actionPanel.getByLabel("處理原因").fill("Full chain wallet freeze");
    await actionPanel.getByRole("button", { name: "提交動作" }).click();
    await expect(
      page.getByText(new RegExp(`提交提款限制 / ${userId} / OPS-FULL-WALLET`)),
    ).toBeVisible();

    await context.addCookies([
      {
        name: SESSION_COOKIE_NAME,
        value: await signSession(
          { sub: "U-ADMIN-004", role: "admin" },
          DEV_SECRET,
        ),
        url: ADMIN_ORIGIN,
        httpOnly: true,
        sameSite: "Strict",
      },
    ]);
    await approvalPanel
      .getByPlaceholder("審批原因")
      .fill("Full chain wallet approval");
    await approvalPanel.getByRole("button", { name: "核准" }).click();
    await expect(
      page.getByText(/effects: member_restriction:withdrawals_locked/),
    ).toBeVisible();
    await expect(
      page.getByText(/wallet_freeze:available_balance/),
    ).toBeVisible();
    await expect(
      page.getByText(/risk_event:member_action_recorded/),
    ).toBeVisible();

    const walletBalance = await fetchJson(
      `${WALLET_LEDGER_URL}/internal/ledger/wallets/${encodeURIComponent(userId)}/balance`,
      { headers: { "x-actor-id": "test", "x-operator-role": "admin" } },
    );
    expect(walletBalance.available_minor_units).toBe(0);
    expect(walletBalance.frozen_minor_units).toBe(-12_500);

    const riskEvents = await fetchJson(
      `${RISK_SERVICE_URL}/internal/risk/member-actions?target_user_id=${encodeURIComponent(userId)}`,
    );
    expect(riskEvents.events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          target_user_id: userId,
          action_type: "restrict_withdrawals",
        }),
      ]),
    );
  } finally {
    await stopServices(services);
  }
});

async function json(response: APIResponse): Promise<any> {
  expect(response.ok()).toBe(true);
  return response.json();
}

function repoRoot(): string {
  return path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
}

function uniqueSchemas() {
  const suffix = Date.now().toString();
  return {
    admin: `pw_admin_${suffix}`,
    user: `pw_user_${suffix}`,
    kyc: `pw_kyc_${suffix}`,
    wallet: `pw_wallet_${suffix}`,
  };
}

function preparePostgresSchemas(schemas: Record<string, string>): void {
  const root = repoRoot();
  const up = spawnSync(
    "docker",
    [
      "compose",
      "-f",
      "infra/docker-compose.local.yaml",
      "up",
      "-d",
      "postgres",
    ],
    { cwd: root, stdio: "inherit" },
  );
  expect(up.status).toBe(0);

  for (const schema of Object.values(schemas)) {
    const result = spawnSync(
      "docker",
      [
        "exec",
        "infra-postgres-1",
        "psql",
        "-U",
        "proph",
        "-d",
        "prophetica",
        "-v",
        "ON_ERROR_STOP=1",
        "-c",
        `DROP SCHEMA IF EXISTS ${schema} CASCADE; CREATE SCHEMA ${schema};`,
      ],
      { cwd: root, stdio: "inherit" },
    );
    expect(result.status).toBe(0);
  }
  runWalletMigrations(schemas.wallet);
}

async function startFullChainServices(
  schemas: Record<string, string>,
): Promise<ChildProcess[]> {
  const root = repoRoot();
  const baseEnv = {
    ...process.env,
    PROPHETICA_ENV: "test",
    RUST_LOG: "warn",
  };
  const core = spawn("cargo", ["run", "-p", "core-api", "--locked"], {
    cwd: root,
    env: {
      ...baseEnv,
      API_GATEWAY_PORT: "43180",
      USER_SERVICE_PORT: "43184",
      WALLET_LEDGER_SERVICE_PORT: "43186",
      MARKET_SERVICE_PORT: "43187",
      POSITION_SERVICE_PORT: "43189",
      RISK_ANTIFRAUD_SERVICE_PORT: "43191",
      NOTIFICATION_SERVICE_PORT: "43193",
      REFERRAL_SERVICE_PORT: "43192",
      USER_SERVICE_STORE: "postgres",
      ADMIN_OPS_DATABASE_URL: dbUrlForSchema(schemas.admin),
      USER_SERVICE_DATABASE_URL: dbUrlForSchema(schemas.user),
      USER_SERVICE_URL: "http://127.0.0.1:43184",
      KYC_SERVICE_URL,
      RISK_SERVICE_URL,
      REFERRAL_SERVICE_BASE_URL: "http://127.0.0.1:43192",
      WALLET_LEDGER_BASE_URL: WALLET_LEDGER_URL,
      MATCHING_ENGINE_BASE_URL: "http://127.0.0.1:9",
    },
    stdio: "pipe",
  });
  pipeServiceLogs(core, "core-api");

  const kyc = spawn(
    "cargo",
    ["run", "-p", "kyc-service-binary", "--bin", "kyc-service", "--locked"],
    {
      cwd: root,
      env: {
        ...baseEnv,
        KYC_SERVICE_PORT: "43185",
        KYC_STORE: "postgres",
        DATABASE_URL: dbUrlForSchema(schemas.kyc),
      },
      stdio: "pipe",
    },
  );
  pipeServiceLogs(kyc, "kyc-service");

  const wallet = spawn(
    "cargo",
    [
      "run",
      "-p",
      "wallet-ledger-service-binary",
      "--bin",
      "wallet-ledger-service",
      "--locked",
    ],
    {
      cwd: root,
      env: {
        ...baseEnv,
        WALLET_LEDGER_SERVICE_PORT: "43186",
        DATABASE_URL: dbUrlForSchema(schemas.wallet),
      },
      stdio: "pipe",
    },
  );
  pipeServiceLogs(wallet, "wallet-ledger-service");

  const risk = spawn(
    "cargo",
    ["run", "-p", "notification-and-risk", "--locked"],
    {
      cwd: root,
      env: {
        ...baseEnv,
        RISK_ANTIFRAUD_SERVICE_PORT: "43191",
        NOTIFICATION_SERVICE_PORT: "43193",
      },
      stdio: "pipe",
    },
  );
  pipeServiceLogs(risk, "notification-and-risk");

  const services = [core, kyc, wallet, risk];
  await Promise.all([
    waitForReady(`${API_GATEWAY_URL}/healthz/ready`),
    waitForReady(`${KYC_SERVICE_URL}/healthz/ready`),
    waitForReady(`${WALLET_LEDGER_URL}/healthz/ready`),
    waitForReady(`${RISK_SERVICE_URL}/healthz/ready`),
  ]);
  return services;
}

function runWalletMigrations(schema: string): void {
  const root = repoRoot();
  const command = `for f in services/wallet-ledger-service/migrations/*.sql; do { echo 'SET search_path TO ${schema};'; cat "$f"; } | docker exec -i infra-postgres-1 psql -U proph -d prophetica -v ON_ERROR_STOP=1; done`;
  const result = spawnSync("bash", ["-lc", command], {
    cwd: root,
    stdio: "inherit",
  });
  expect(result.status).toBe(0);
}

function dbUrlForSchema(schema: string): string {
  const separator = DATABASE_URL.includes("?") ? "&" : "?";
  return `${DATABASE_URL}${separator}options=-csearch_path%3D${schema}`;
}

async function seedWalletBalance(
  userId: string,
  amountMinorUnits: number,
): Promise<void> {
  const response = await fetch(
    `${WALLET_LEDGER_URL}/internal/ledger/transactions`,
    {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "idempotency-key": `full-chain-seed:${userId}`,
      },
      body: JSON.stringify({
        txn_type: "deposit",
        correlation_id: `full-chain-seed:${userId}`,
        description: "full chain wallet seed",
        entries: [
          {
            account_code: "Bank.TrustAccount.TWD",
            direction: "debit",
            amount_minor_units: amountMinorUnits,
          },
          {
            account_code: `Player.Wallet.${userId}`,
            direction: "credit",
            amount_minor_units: amountMinorUnits,
          },
        ],
      }),
    },
  );
  expect(response.ok).toBe(true);
}

async function fetchJson(url: string, init?: RequestInit): Promise<any> {
  const response = await fetch(url, init);
  expect(response.ok).toBe(true);
  return response.json();
}

async function waitForReady(url: string): Promise<void> {
  const started = Date.now();
  while (Date.now() - started < 180_000) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // service is still starting
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

function pipeServiceLogs(child: ChildProcess, name: string): void {
  child.stdout?.on("data", (chunk) =>
    process.stdout.write(`[${name}] ${chunk}`),
  );
  child.stderr?.on("data", (chunk) =>
    process.stderr.write(`[${name}] ${chunk}`),
  );
}

async function stopServices(services: ChildProcess[]): Promise<void> {
  await Promise.all(
    services.map(
      (service) =>
        new Promise<void>((resolve) => {
          if (service.exitCode !== null) {
            resolve();
            return;
          }
          service.once("exit", () => resolve());
          service.kill("SIGTERM");
          setTimeout(() => {
            if (service.exitCode === null) service.kill("SIGKILL");
            resolve();
          }, 5_000);
        }),
    ),
  );
}
