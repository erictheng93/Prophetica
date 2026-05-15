import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { beforeEach, test } from "node:test";
import {
  listMemberPendingCases,
  resetMemberManagementStoreForTests,
} from "../lib/member-management.js";
import { allowsMemberMockFallback } from "../lib/member-management-runtime.js";

beforeEach(() => {
  resetMemberManagementStoreForTests();
  delete process.env.API_GATEWAY_URL;
  delete process.env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK;
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
  assert.equal(allowsMemberMockFallback({ PROPHETICA_ENV: "dev" }), false);
  assert.equal(
    allowsMemberMockFallback({ MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK: "true" }),
    true,
  );
});

test("member action BFF routes through api-gateway before dev fallback", () => {
  const source = readFileSync("lib/member-management-bff.ts", "utf8");
  const actionStart = source.indexOf("export async function proxyMemberActionRequest");
  const gatewayCall = source.indexOf(
    'adminPath: "/api/internal/admin/members/actions"',
    actionStart,
  );
  const fallbackCall = source.indexOf("findMemberSummary(parsed.value.target_user_id", actionStart);

  assert.notEqual(actionStart, -1);
  assert.notEqual(gatewayCall, -1);
  assert.notEqual(fallbackCall, -1);
  assert.ok(gatewayCall < fallbackCall);
  assert.match(source, /gateway_unconfigured/);
});
