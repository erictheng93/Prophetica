import { expect, test, type Page } from "@playwright/test";
import { spawn, type ChildProcess } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ADMIN_ORIGIN, ADMIN_PORT } from "../admin-env.js";
import { signSession, verifySession } from "../../lib/session-crypto.js";

const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";
const SESSION_COOKIE_NAME = "proph_admin_session";
const USER_SERVICE_URL = "http://127.0.0.1:43204";

test("admin passkey UX completes browser WebAuthn ceremony and records P0 evidence markers", async ({
  browserName,
  context,
  page,
}) => {
  test.skip(browserName !== "chromium", "CDP virtual authenticators require Chromium");

  const markers: string[] = [];
  const core = startCoreApi();
  await waitForReady(`${USER_SERVICE_URL}/healthz/ready`);

  const cdp = await context.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "usb",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

  let registerFinishBody: unknown;
  let assertionFinishBody: unknown;
  let registeredCredentialId = "";
  const diagnostics: string[] = [];
  page.on("console", (message) => diagnostics.push(`console:${message.type()}:${message.text()}`));
  page.on("pageerror", (error) => diagnostics.push(`pageerror:${error.message}`));
  page.on("requestfailed", (request) =>
    diagnostics.push(`requestfailed:${request.method()}:${request.url()}:${request.failure()?.errorText}`),
  );
  page.on("request", (request) => {
    if (request.method() !== "POST") return;
    if (request.url().includes("/api/auth/passkeys/register-finish")) {
      registerFinishBody = request.postDataJSON();
    }
    if (request.url().includes("/api/auth/passkeys/assert-finish")) {
      assertionFinishBody = request.postDataJSON();
    }
  });
  page.on("response", async (response) => {
    let responseText = "";
    if (response.url().includes("/api/auth/passkeys/")) {
      responseText = await response.text().catch(() => "");
      diagnostics.push(`response:${response.status()}:${response.url()}:${responseText}`);
    }
    if (!response.url().includes("/api/auth/passkeys/register-finish") || !response.ok()) return;
    const body = JSON.parse(responseText || "{}") as { credential_id?: string };
    registeredCredentialId = body.credential_id ?? "";
  });

  try {
    await login(page, ADMIN_ORIGIN);

    await expect(page.getByRole("button", { name: "註冊這台裝置的 passkey" })).toBeEnabled();
    const registration = await registerPasskeyInBrowser(page);
    registeredCredentialId = String(registration.credential_id ?? registeredCredentialId);
    expect(registeredCredentialId).not.toEqual("");
    markers.push("registration_round_trip=true");

    const registrationReplay = await page.request.post("/api/auth/passkeys/register-finish", {
      data: registerFinishBody,
    });
    expect(registrationReplay.ok()).toBe(false);
    markers.push("registration_replay_rejected=true");

    await assertPasskeyInBrowser(page);
    markers.push("assertion_round_trip=true");

    const assertionReplay = await page.request.post("/api/auth/passkeys/assert-finish", {
      data: assertionFinishBody,
    });
    expect(assertionReplay.ok()).toBe(false);
    markers.push("assertion_replay_rejected=true");

    const unknownAssertionStart = await page.request.post("/api/auth/passkeys/assert-start");
    expect(unknownAssertionStart.ok()).toBe(true);
    const unknownAssertionChallenge = (await unknownAssertionStart.json()) as {
      challenge_id: string;
      request_options: unknown;
    };
    const unknownAssertionData = await createAssertion(page, unknownAssertionChallenge.request_options);
    unknownAssertionData.id = "unknown-credential";
    unknownAssertionData.rawId = "dW5rbm93bi1jcmVkZW50aWFs";
    const unknownAssertionFinish = await page.request.post("/api/auth/passkeys/assert-finish", {
      data: {
        challenge_id: unknownAssertionChallenge.challenge_id,
        assertion_data: unknownAssertionData,
      },
    });
    expect(unknownAssertionFinish.ok()).toBe(false);
    markers.push("unknown_credential_rejected=true");

    const sessionCookie = (await context.cookies(ADMIN_ORIGIN)).find(
      (cookie) => cookie.name === SESSION_COOKIE_NAME,
    );
    expect(sessionCookie?.value).toBeTruthy();
    const session = await verifySession(sessionCookie?.value ?? "", DEV_SECRET);
    expect(session?.amr).toContain("hwk");
    expect(typeof session?.mfaAt).toBe("number");
    markers.push("fresh_hardware_mfa_session=true");

    const freshStepUp = await page.request.post("/api/points/manual-adjustments", {
      data: {},
    });
    expect(freshStepUp.status()).not.toBe(403);
    markers.push("admin_step_up_fresh_accepts=true");

    await context.addCookies([
      {
        name: SESSION_COOKIE_NAME,
        value: await signSession(
          {
            sub: "U-WEBAUTHN-E2E-001",
            role: "admin",
            mfaAt: Math.floor(Date.now() / 1000) - 31 * 60,
            amr: ["pwd", "hwk"],
          },
          DEV_SECRET,
        ),
        url: ADMIN_ORIGIN,
        httpOnly: true,
        sameSite: "Strict",
      },
    ]);
    const staleStepUp = await page.request.post("/api/points/manual-adjustments", {
      data: {},
    });
    expect(staleStepUp.status()).toBe(403);
    markers.push("admin_step_up_stale_rejects=true");

    const wrongOrigin = `http://127.0.0.1:${ADMIN_PORT}`;
    const authenticatedCookies = await context.cookies(ADMIN_ORIGIN);
    await context.addCookies(
      authenticatedCookies.map((cookie) => ({
        name: cookie.name,
        value: cookie.value,
        url: wrongOrigin,
        expires: cookie.expires,
        httpOnly: cookie.httpOnly,
        secure: cookie.secure,
        sameSite: cookie.sameSite,
      })),
    );
    await page.goto(`${wrongOrigin}/login`);
    await registerPasskeyInBrowser(page).then(
      () => {
        throw new Error("wrong_origin_registration_unexpectedly_succeeded");
      },
      () => undefined,
    );
    markers.push("wrong_origin_rejected=true");

    await login(page, ADMIN_ORIGIN);
    const revokeResponse = await page.request.post("/api/auth/passkeys/revoke", {
      data: { credential_id: registeredCredentialId },
    });
    expect(revokeResponse.ok()).toBe(true);
    const revokedAssertStart = await page.request.post("/api/auth/passkeys/assert-start");
    expect(revokedAssertStart.ok()).toBe(false);
    markers.push("revoked_credential_rejected=true");
  } finally {
    await stopService(core);
    writeEvidenceLog(markers);
  }
});

async function login(page: Page, origin: string): Promise<void> {
  await page.context().clearCookies();
  await page.goto(`${origin}/login`);
  await page.getByLabel("電子郵件").fill("webauthn-admin@prophetica.local");
  await page.getByLabel("密碼").fill("admin");
  await page.getByRole("button", { name: "登入" }).click();
  await expect(page.getByRole("heading", { name: "管理操作入口" })).toBeVisible();
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(5_000);
  await expect(page.getByRole("heading", { name: "管理操作入口" })).toBeVisible();
}

async function createAssertion(page: Page, requestOptions: unknown): Promise<Record<string, unknown>> {
  return page.evaluate(async (options) => {
    function decode(value: string): ArrayBuffer {
      const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
      const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), "=");
      const binary = atob(padded);
      const bytes = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
      }
      return bytes.buffer;
    }
    function encode(buffer: ArrayBuffer): string {
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (const byte of bytes) {
        binary += String.fromCharCode(byte);
      }
      return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    }
    const container = JSON.parse(JSON.stringify(options ?? {}));
    const publicKey = container.publicKey ?? container.public_key ?? container;
    publicKey.challenge = decode(String(publicKey.challenge ?? ""));
    if (Array.isArray(publicKey.allowCredentials)) {
      publicKey.allowCredentials = publicKey.allowCredentials.map((credential: Record<string, unknown>) => ({
        ...credential,
        id: decode(String(credential.id ?? "")),
      }));
    }
    const credential = (await navigator.credentials.get({ publicKey })) as PublicKeyCredential;
    const response = credential.response as AuthenticatorAssertionResponse;
    return {
      id: credential.id,
      rawId: encode(credential.rawId),
      type: credential.type,
      response: {
        clientDataJSON: encode(response.clientDataJSON),
        authenticatorData: encode(response.authenticatorData),
        signature: encode(response.signature),
        userHandle: response.userHandle ? encode(response.userHandle) : null,
      },
      clientExtensionResults: credential.getClientExtensionResults(),
      authenticatorAttachment: credential.authenticatorAttachment,
    };
  }, requestOptions);
}

async function registerPasskeyInBrowser(page: Page): Promise<Record<string, unknown>> {
  return page.evaluate(async () => {
    function decode(value: string): ArrayBuffer {
      const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
      const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), "=");
      const binary = atob(padded);
      const bytes = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
      }
      return bytes.buffer;
    }
    function encode(buffer: ArrayBuffer): string {
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (const byte of bytes) {
        binary += String.fromCharCode(byte);
      }
      return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    }
    function decodeCreationOptions(value: Record<string, unknown>): PublicKeyCredentialCreationOptions {
      const container = JSON.parse(JSON.stringify(value ?? {}));
      const publicKey = container.publicKey ?? container.public_key ?? container;
      publicKey.challenge = decode(String(publicKey.challenge ?? ""));
      if (publicKey.user && typeof publicKey.user === "object") {
        publicKey.user.id = decode(String(publicKey.user.id ?? ""));
      }
      if (Array.isArray(publicKey.excludeCredentials)) {
        publicKey.excludeCredentials = publicKey.excludeCredentials.map((credential: Record<string, unknown>) => ({
          ...credential,
          id: decode(String(credential.id ?? "")),
        }));
      }
      return publicKey;
    }
    function serializeCredential(credential: PublicKeyCredential): Record<string, unknown> {
      const response = credential.response as AuthenticatorAttestationResponse;
      return {
        id: credential.id,
        rawId: encode(credential.rawId),
        type: credential.type,
        response: {
          clientDataJSON: encode(response.clientDataJSON),
          attestationObject: encode(response.attestationObject),
          transports: response.getTransports(),
        },
        clientExtensionResults: credential.getClientExtensionResults(),
        authenticatorAttachment: credential.authenticatorAttachment,
      };
    }

    const startResponse = await fetch("/api/auth/passkeys/register-start", { method: "POST" });
    const start = await startResponse.json();
    if (!startResponse.ok) throw new Error(JSON.stringify(start));

    const publicKey = decodeCreationOptions(start.creation_options);
    const credential = (await navigator.credentials.create({ publicKey })) as PublicKeyCredential | null;
    if (!credential) throw new Error("passkey_registration_cancelled");

    const finishResponse = await fetch("/api/auth/passkeys/register-finish", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        challenge_id: start.challenge_id,
        registration_data: serializeCredential(credential),
      }),
    });
    const finish = await finishResponse.json().catch(() => ({}));
    if (!finishResponse.ok) throw new Error(JSON.stringify(finish));
    return finish as Record<string, unknown>;
  });
}

async function assertPasskeyInBrowser(page: Page): Promise<Record<string, unknown>> {
  return page.evaluate(async () => {
    function decode(value: string): ArrayBuffer {
      const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
      const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), "=");
      const binary = atob(padded);
      const bytes = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
      }
      return bytes.buffer;
    }
    function encode(buffer: ArrayBuffer): string {
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (const byte of bytes) {
        binary += String.fromCharCode(byte);
      }
      return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    }
    function decodeRequestOptions(value: Record<string, unknown>): PublicKeyCredentialRequestOptions {
      const container = JSON.parse(JSON.stringify(value ?? {}));
      const publicKey = container.publicKey ?? container.public_key ?? container;
      publicKey.challenge = decode(String(publicKey.challenge ?? ""));
      if (Array.isArray(publicKey.allowCredentials)) {
        publicKey.allowCredentials = publicKey.allowCredentials.map((credential: Record<string, unknown>) => ({
          ...credential,
          id: decode(String(credential.id ?? "")),
        }));
      }
      return publicKey;
    }
    function serializeCredential(credential: PublicKeyCredential): Record<string, unknown> {
      const response = credential.response as AuthenticatorAssertionResponse;
      return {
        id: credential.id,
        rawId: encode(credential.rawId),
        type: credential.type,
        response: {
          clientDataJSON: encode(response.clientDataJSON),
          authenticatorData: encode(response.authenticatorData),
          signature: encode(response.signature),
          userHandle: response.userHandle ? encode(response.userHandle) : null,
        },
        clientExtensionResults: credential.getClientExtensionResults(),
        authenticatorAttachment: credential.authenticatorAttachment,
      };
    }

    const startResponse = await fetch("/api/auth/passkeys/assert-start", { method: "POST" });
    const start = await startResponse.json();
    if (!startResponse.ok) throw new Error(JSON.stringify(start));

    const publicKey = decodeRequestOptions(start.request_options);
    const credential = (await navigator.credentials.get({ publicKey })) as PublicKeyCredential | null;
    if (!credential) throw new Error("passkey_assertion_cancelled");

    const finishResponse = await fetch("/api/auth/passkeys/assert-finish", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        challenge_id: start.challenge_id,
        assertion_data: serializeCredential(credential),
      }),
    });
    const finish = await finishResponse.json().catch(() => ({}));
    if (!finishResponse.ok) throw new Error(JSON.stringify(finish));
    return finish as Record<string, unknown>;
  });
}

function startCoreApi(): ChildProcess {
  const environment: NodeJS.ProcessEnv = {
    CARGO_HOME: process.env.CARGO_HOME,
    HOME: process.env.HOME,
    NODE_ENV: process.env.NODE_ENV ?? "test",
    PATH: process.env.PATH,
    RUSTUP_HOME: process.env.RUSTUP_HOME,
    TMPDIR: process.env.TMPDIR,
  };

  const core = spawn("cargo", ["run", "-p", "core-api", "--locked"], {
    cwd: repoRoot(),
    env: {
      ...environment,
      PROPHETICA_ENV: "test",
      RUST_LOG: "warn",
      ADMIN_OPS_DATABASE_URL: "",
      DATABASE_URL: "",
      REFERRAL_SERVICE_DATABASE_URL: "",
      USER_SERVICE_DATABASE_URL: "",
      USER_SERVICE_STORE: "memory",
      REFERRAL_SERVICE_STORE: "memory",
      API_GATEWAY_PORT: "43200",
      USER_SERVICE_PORT: "43204",
      MARKET_SERVICE_PORT: "43207",
      POSITION_SERVICE_PORT: "43209",
      REFERRAL_SERVICE_PORT: "43212",
      USER_SERVICE_URL,
      USER_SERVICE_WEBAUTHN_RP_ID: "localhost",
      USER_SERVICE_WEBAUTHN_RP_ORIGIN: ADMIN_ORIGIN,
      USER_SERVICE_LOCAL_LOGIN_ACCOUNTS: JSON.stringify([
        {
          user_id: "U-WEBAUTHN-E2E-001",
          email: "webauthn-admin@prophetica.local",
          phone: "0912345614",
          password: "admin",
          role: "admin",
          webauthn_user_uuid: "018f6f70-1c40-7000-8000-000000000004",
        },
      ]),
    },
    stdio: "pipe",
  }) as ChildProcess;
  core.stdout?.on("data", (chunk: Buffer) =>
    process.stdout.write(`[core-api] ${chunk}`),
  );
  core.stderr?.on("data", (chunk: Buffer) =>
    process.stderr.write(`[core-api] ${chunk}`),
  );
  return core;
}

async function waitForReady(url: string): Promise<void> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 300_000) {
    const response = await fetch(url).catch(() => null);
    if (response?.ok) return;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function stopService(process: ChildProcess): Promise<void> {
  if (process.killed || process.exitCode !== null) return;
  process.kill("SIGINT");
  await new Promise<void>((resolve) => {
    const timeout = setTimeout(resolve, 5_000);
    process.once("exit", () => {
      clearTimeout(timeout);
      resolve();
    });
  });
}

function writeEvidenceLog(markers: string[]): void {
  const output = path.join(repoRoot(), ".tmp", "webauthn-passkey.log");
  mkdirSync(path.dirname(output), { recursive: true });
  writeFileSync(output, `${markers.join("\n")}\n`);
}

function repoRoot(): string {
  return path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
}
