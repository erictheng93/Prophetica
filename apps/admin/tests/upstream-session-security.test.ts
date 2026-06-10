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

  assert.match(loginRoute, /setUserServiceSessionCookie\(user\.session_token\)/);
  assert.doesNotMatch(loginRoute, /setSessionCookie\(\{/);
  assert.match(loginRoute, /login_surface: loginSurface/);
  assert.match(loginRoute, /mfa_code: optionalString\(body\.mfa_code\)/);
  assert.match(loginRoute, /mfa_recovery_code: optionalString\(body\.mfa_recovery_code\)/);
  assert.match(loginRoute, /affiliate_portal/);
  assert.match(loginRoute, /cf-connecting-ip/);
  assert.match(loginRoute, /x-forwarded-for/);
  assert.match(logoutRoute, /clearUpstreamSessionCookie\(\)/);
  assert.match(upstreamSession, /httpOnly: true/);
  assert.match(upstreamSession, /sameSite: "strict"/);
});
