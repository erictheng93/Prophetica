import assert from "node:assert/strict";
import { test } from "node:test";
import { findActiveNavChildKey } from "../lib/admin-nav-active.js";

const affiliateChildren = [
  { key: "affiliate-list", href: "/affiliate" },
  { key: "affiliate-commissions", href: "/affiliate/commissions" },
  { key: "affiliate-payouts", href: "/affiliate/payouts" },
] as const;

test("affiliate child navigation picks the most specific active route", () => {
  assert.equal(
    findActiveNavChildKey("/affiliate/commissions", affiliateChildren),
    "affiliate-commissions",
  );
  assert.equal(
    findActiveNavChildKey("/affiliate/payouts", affiliateChildren),
    "affiliate-payouts",
  );
});

test("affiliate list stays active for the list page and affiliate detail pages", () => {
  assert.equal(
    findActiveNavChildKey("/affiliate", affiliateChildren),
    "affiliate-list",
  );
  assert.equal(
    findActiveNavChildKey("/affiliate/AFF-001", affiliateChildren),
    "affiliate-list",
  );
});
