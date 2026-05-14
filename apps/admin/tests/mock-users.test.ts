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

test("login development hint points operators to user-service local-login configuration", () => {
  assert.match(adminCopy.login.devHintBody, /user-service/);
  assert.match(
    adminCopy.login.devHintBody,
    /USER_SERVICE_LOCAL_LOGIN_ACCOUNTS|USER_SERVICE_LOCAL_LOGIN_EMAIL\/PASSWORD\/ROLE/,
  );
  assert.match(adminCopy.login.devHintBody, /admin、manager 或 affiliate/);
  assert.doesNotMatch(adminCopy.login.devHintBody, /Phase 2/);
});
