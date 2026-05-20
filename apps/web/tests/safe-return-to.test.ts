import assert from "node:assert/strict";
import test from "node:test";

import { safeReturnTo } from "../lib/safe-return-to.js";

test("safeReturnTo accepts same-origin relative deep links with query strings", () => {
  assert.equal(
    safeReturnTo("/markets/abc?side=yes&qty=10"),
    "/markets/abc?side=yes&qty=10",
  );
});

test("safeReturnTo falls back for missing or empty targets", () => {
  assert.equal(safeReturnTo(undefined), "/portfolio");
  assert.equal(safeReturnTo(""), "/portfolio");
  assert.equal(safeReturnTo("   "), "/portfolio");
});

test("safeReturnTo rejects absolute and protocol-relative URLs", () => {
  assert.equal(safeReturnTo("https://evil.example/x"), "/portfolio");
  assert.equal(safeReturnTo("http://evil.example/x"), "/portfolio");
  assert.equal(safeReturnTo("//evil.example/x"), "/portfolio");
});

test("safeReturnTo rejects encoded cross-domain absolute URLs", () => {
  assert.equal(safeReturnTo("https%3A%2F%2Fevil.example%2Fx"), "/portfolio");
});

test("safeReturnTo rejects CRLF and control character injection", () => {
  assert.equal(safeReturnTo("/markets/abc%0d%0aLocation:%20/x"), "/portfolio");
  assert.equal(safeReturnTo("/markets/abc\nSet-Cookie: x=y"), "/portfolio");
});

test("safeReturnTo rejects overly long targets", () => {
  assert.equal(safeReturnTo(`/${"a".repeat(2048)}`), "/portfolio");
});

test("safeReturnTo uses caller fallback when supplied", () => {
  assert.equal(safeReturnTo("https://evil.example/x", "/"), "/");
});
