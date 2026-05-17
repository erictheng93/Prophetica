import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const readAdminFile = (path: string) =>
  readFileSync(join(process.cwd(), path), "utf8");

test("admin login and logout preserve the upstream user-service session in an httpOnly cookie", () => {
  const loginRoute = readAdminFile("app/api/auth/login/route.ts");
  const logoutRoute = readAdminFile("app/api/auth/logout/route.ts");
  const upstreamSession = readAdminFile("lib/upstream-session.ts");

  assert.match(loginRoute, /setUpstreamSessionCookie\(user\.session_token\)/);
  assert.match(logoutRoute, /clearUpstreamSessionCookie\(\)/);
  assert.match(upstreamSession, /httpOnly: true/);
  assert.match(upstreamSession, /sameSite: "strict"/);
});

test("admin passkey proxy forwards WebAuthn ceremonies to user-service with bearer auth", () => {
  const route = readAdminFile("app/api/auth/passkeys/[action]/route.ts");

  assert.match(route, /"register-start"/);
  assert.match(route, /"register-finish"/);
  assert.match(route, /"assert-start"/);
  assert.match(route, /"assert-finish"/);
  assert.match(route, /"revoke"/);
  assert.match(route, /\/v1\/auth\/webauthn\/credentials\/revoke/);
  assert.match(route, /authorization: `Bearer \$\{upstreamSessionToken\}`/);
  assert.match(route, /setUpstreamSessionCookie\(response\.session_token\)/);
  assert.match(route, /amr: \["pwd", "hwk"\]/);
});

test("admin passkey client performs browser ceremonies without exposing bearer tokens", () => {
  const client = readAdminFile("lib/passkey-client.ts");
  const panel = readAdminFile("components/passkey-security-panel.tsx");
  const page = readAdminFile("app/page.tsx");

  assert.match(client, /navigator\.credentials\.create/);
  assert.match(client, /navigator\.credentials\.get/);
  assert.match(client, /base64UrlToArrayBuffer/);
  assert.match(client, /arrayBufferToBase64Url/);
  assert.doesNotMatch(client, /Bearer/);
  assert.match(panel, /registerPasskey/);
  assert.match(panel, /assertPasskey/);
  assert.match(page, /<PasskeySecurityPanel \/>/);
});
