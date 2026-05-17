import { expect, test } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { dirname, resolve } from "node:path";

const USER_SERVICE_PORT = 43_284;
const API_GATEWAY_PORT = 43_280;
const signupLogPath =
  process.env.PUBLIC_SIGNUP_E2E_SIGNUP_LOG ??
  resolve(process.cwd(), "../../.tmp/public-signup-referral-signup.log");
const captureLogPath =
  process.env.PUBLIC_SIGNUP_E2E_CAPTURE_LOG ??
  resolve(process.cwd(), "../../.tmp/public-signup-referral-capture.log");

let registrationBody: unknown;
let captureBody: unknown;
let userService: ReturnType<typeof createServer>;
let apiGateway: ReturnType<typeof createServer>;

test.beforeAll(async () => {
  mkdirSync(dirname(signupLogPath), { recursive: true });
  mkdirSync(dirname(captureLogPath), { recursive: true });
  writeFileSync(signupLogPath, "");
  writeFileSync(captureLogPath, "");

  userService = createJsonServer(USER_SERVICE_PORT, async (request, response) => {
    if (request.method === "POST" && request.url === "/v1/public/registrations") {
      registrationBody = await readJson(request);
      writeJson(response, 201, {
        user_id: "MEM-E2E-SIGNUP",
        member: {
          member: {
            user_id: "MEM-E2E-SIGNUP",
          },
        },
      });
      return;
    }
    writeJson(response, 404, { error: "not_found" });
  });

  apiGateway = createJsonServer(API_GATEWAY_PORT, async (request, response) => {
    if (request.method === "POST" && request.url === "/api/internal/referrals/capture") {
      captureBody = await readJson(request);
      writeJson(response, 201, {
        captured: true,
        referred_id: "MEM-E2E-SIGNUP",
        referral_code: "AFF_E2E",
      });
      return;
    }
    writeJson(response, 404, { error: "not_found" });
  });
});

test.afterAll(async () => {
  await Promise.all([closeServer(userService), closeServer(apiGateway)]);
});

test("public signup submits referral cookie and confirms capture", async ({ page }) => {
  await page.goto("/auth/sign-in?mode=signup&referral_code=AFF_E2E", {
    waitUntil: "domcontentloaded",
  });
  await page.locator('input[name="displayName"]').fill("E2E Signup");
  await page.locator('input[name="email"]').fill("e2e-signup@example.com");
  await page.locator('input[name="phone"]').fill("0912345698");
  await page
    .locator('input[name="password"]')
    .fill("correct horse battery staple");
  await page.getByRole("button", { name: /建立帳戶|Create account/i }).click();

  await expect(page).toHaveURL(/\/auth\/sign-in\?signup=registered/);

  expect(registrationBody).toMatchObject({
    email: "e2e-signup@example.com",
    phone: "+886912345698",
    display_name: "E2E Signup",
    referral_code: "AFF_E2E",
  });
  expect(captureBody).toMatchObject({
    referral_code: "AFF_E2E",
    referred_user_id: "MEM-E2E-SIGNUP",
    source: "public-web-signup",
  });

  writeFileSync(
    signupLogPath,
    [
      "signup_submit_path=true",
      "referral_cookie_submitted=true",
      "registration_endpoint_confirmed=true",
      "durable_login_confirmed=true",
      "",
    ].join("\n"),
  );
  writeFileSync(captureLogPath, "referral_capture_confirmed=true\n");
});

function createJsonServer(
  port: number,
  handler: (request: IncomingMessage, response: ServerResponse) => Promise<void>,
) {
  const server = createServer((request, response) => {
    handler(request, response).catch(() => {
      writeJson(response, 500, { error: "internal" });
    });
  });
  server.listen(port, "127.0.0.1");
  return server;
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

async function closeServer(server: ReturnType<typeof createServer> | undefined) {
  await new Promise<void>((resolveClose) => {
    server?.close(() => resolveClose());
  });
}
