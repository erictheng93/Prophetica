import assert from "node:assert/strict";
import { test } from "node:test";

import { memberLookupErrorMessage } from "../lib/member-lookup-error.js";

const NOT_FOUND_MESSAGE = "查無此會員，請確認輸入的會員 ID 是否正確。";
const UNAVAILABLE_MESSAGE = "查詢服務暫時無法使用，請稍後再試。";

test("404 with a known member-not-found code shows the not-found message", () => {
  // Gateway emits ERROR_CODE_MEMBER_NOT_FOUND (SPEC-014 §6.1.1); the BFF's own
  // mock-fallback / affiliate-scope path emits member_not_found. Both are 404s
  // that mean "member not found" and must surface the same message.
  assert.equal(
    memberLookupErrorMessage(404, "ERROR_CODE_MEMBER_NOT_FOUND"),
    NOT_FOUND_MESSAGE,
  );
  assert.equal(
    memberLookupErrorMessage(404, "member_not_found"),
    NOT_FOUND_MESSAGE,
  );
});

test("404 with an unparseable body falls back to the not-found message", () => {
  // A 404 on the member-summary endpoint always means not-found; when the body
  // carries no readable code, the status alone is a safe fallback.
  assert.equal(memberLookupErrorMessage(404, null), NOT_FOUND_MESSAGE);
});

test("404 with an unrelated error code shows the generic message", () => {
  // Proves the code is actually consulted, not just the status.
  assert.equal(
    memberLookupErrorMessage(404, "ERROR_CODE_SOMETHING_ELSE"),
    UNAVAILABLE_MESSAGE,
  );
});

test("502 and other 5xx show the generic unavailable message", () => {
  assert.equal(
    memberLookupErrorMessage(502, "ERROR_CODE_USER_SERVICE_UNAVAILABLE"),
    UNAVAILABLE_MESSAGE,
  );
  assert.equal(memberLookupErrorMessage(500, null), UNAVAILABLE_MESSAGE);
});
