import assert from "node:assert/strict";
import { test } from "node:test";
import {
  bypassesCloudflareAccess,
  requiresCloudflareAccess,
} from "../lib/cloudflare-access-policy.js";

test("cloudflare access bypass is limited to login and affiliate portal", () => {
  for (const path of [
    "/login",
    "/login/help",
    "/me/affiliate",
    "/me/affiliate/points",
    "/me/affiliate/payouts",
  ]) {
    assert.equal(bypassesCloudflareAccess(path), true, path);
    assert.equal(requiresCloudflareAccess(path), false, path);
  }
});

test("internal admin surfaces still require cloudflare access", () => {
  for (const path of [
    "/",
    "/dashboard",
    "/members",
    "/affiliate",
    "/analytics",
    "/kyc-risk",
    "/compensation",
    "/approvals",
  ]) {
    assert.equal(requiresCloudflareAccess(path), true, path);
    assert.equal(bypassesCloudflareAccess(path), false, path);
  }
});
