import assert from "node:assert/strict";
import { test } from "node:test";
import { adminCopy } from "../lib/admin-copy.js";
import { findMockUser, mockUsers } from "../lib/mock-users-data.js";

test("mock users include two managers and two affiliates", () => {
  assert.equal(mockUsers.filter((user) => user.role === "manager").length, 2);
  assert.equal(mockUsers.filter((user) => user.role === "affiliate").length, 2);
});

test("second affiliate login maps to the AFF-014 mock affiliate", () => {
  const user = findMockUser("affiliate2@prophetica.local", "affiliate2");

  assert.equal(user?.sub, "U-AFF-002");
  assert.equal(user?.role, "affiliate");
  assert.equal(user?.affiliateId, "AFF-014");
});

test("login development hint lists every mock account without an incorrect shared password claim", () => {
  for (const user of mockUsers) {
    assert.match(adminCopy.login.devHintBody, new RegExp(user.email.replaceAll(".", "\\.")));
  }
  assert.doesNotMatch(adminCopy.login.devHintBody, /密碼皆為 dev/);
});
