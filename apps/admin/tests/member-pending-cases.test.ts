import assert from "node:assert/strict";
import { beforeEach, test } from "node:test";
import {
  listMemberPendingCases,
  resetMemberManagementStoreForTests,
} from "../lib/member-management.js";
import { allowsMemberMockFallback } from "../lib/member-management-runtime.js";

beforeEach(() => {
  resetMemberManagementStoreForTests();
});

test("pending member cases expose visual columns and pagination", () => {
  const result = listMemberPendingCases({
    page: 1,
    pageSize: 1,
    status: "restricted",
  });

  assert.equal(result.page, 1);
  assert.equal(result.page_size, 1);
  assert.equal(result.total, 1);
  assert.equal(result.items[0]?.member_id, "MEM-1042");
  assert.equal(result.items[0]?.restriction_reason, "withdrawals_locked");
  assert.equal(result.items[0]?.contact.email_masked, "c***@example.net");
  assert.equal(result.items[0]?.status, "restricted");
});

test("pending member cases can filter by member id or contact text", () => {
  const byMember = listMemberPendingCases({ query: "1042" });
  const byContact = listMemberPendingCases({ query: "0900" });

  assert.deepEqual(
    byMember.items.map((item) => item.member_id),
    ["MEM-1042"],
  );
  assert.deepEqual(
    byContact.items.map((item) => item.member_id),
    ["user_123"],
  );
});

test("pending member cases mock fallback is disabled in production", () => {
  assert.equal(allowsMemberMockFallback({ PROPHETICA_ENV: "production" }), false);
  assert.equal(allowsMemberMockFallback({ NODE_ENV: "production" }), false);
  assert.equal(allowsMemberMockFallback({ PROPHETICA_ENV: "dev" }), true);
});
