import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { resolveLoginRedirect } from "../lib/login-redirect.js";

const readAdminFile = (path: string) =>
  readFileSync(join(process.cwd(), path), "utf8");

test("login redirect prefers a safe intended path over the API fallback", () => {
  assert.equal(resolveLoginRedirect("/dashboard", "/"), "/dashboard");
  assert.equal(
    resolveLoginRedirect("/analytics/jobs?status=ready", "/"),
    "/analytics/jobs?status=ready",
  );
});

test("login redirect rejects external, protocol-relative, and script destinations", () => {
  assert.equal(resolveLoginRedirect("https://example.invalid", "/"), "/");
  assert.equal(resolveLoginRedirect("//example.invalid/path", "/"), "/");
  assert.equal(resolveLoginRedirect("javascript:alert(1)", "/"), "/");
});

test("login redirect falls back to root when both destinations are unsafe", () => {
  assert.equal(resolveLoginRedirect(null, "https://example.invalid"), "/");
  assert.equal(resolveLoginRedirect("", ""), "/");
});

test("login form uses document navigation after a successful auth cookie response", () => {
  const source = readAdminFile("components/login-form.tsx");

  assert.match(source, /window\.location\.assign/);
  assert.doesNotMatch(source, /router\.replace\(intendedRedirect/);
});

test("login form forwards optional MFA step-up evidence", () => {
  const source = readAdminFile("components/login-form.tsx");
  const route = readAdminFile("app/api/auth/login/route.ts");

  assert.match(source, /const \[mfaCode, setMfaCode\]/);
  assert.match(source, /const \[mfaRecoveryCode, setMfaRecoveryCode\]/);
  assert.match(source, /autoComplete="one-time-code"/);
  assert.match(source, /pattern="\[0-9\]\{6\}"/);
  assert.match(source, /JSON\.stringify\(\{ email, password, mfaCode, mfaRecoveryCode \}\)/);
  assert.match(route, /mfa_code: mfaCode/);
  assert.match(route, /mfa_recovery_code: mfaRecoveryCode/);
});
