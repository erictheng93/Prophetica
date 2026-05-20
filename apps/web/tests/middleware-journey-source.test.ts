import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const source = readFileSync(join(process.cwd(), "middleware.ts"), "utf8");

test("middleware declares SPEC-023 protected and public journey path sets", () => {
  assert.match(source, /const PROTECTED_PATH_PREFIXES = \[/);
  for (const path of ["/portfolio", "/topup", "/kyc", "/account"]) {
    assert.match(source, new RegExp(JSON.stringify(path)));
  }
  assert.match(source, /const PUBLIC_PATH_PREFIXES = \[/);
  for (const path of [
    "/markets",
    "/legal",
    "/support",
    "/how-it-works",
    "/auth",
  ]) {
    assert.match(source, new RegExp(JSON.stringify(path)));
  }
});

test("middleware redirects protected anonymous requests with same-origin return_to", () => {
  assert.match(source, /SESSION_COOKIE_NAME/);
  assert.match(
    source,
    /redirectUrl\.searchParams\.set\("return_to", returnTo\);/,
  );
  assert.match(source, /NextResponse\.redirect\(redirectUrl\)/);
});
