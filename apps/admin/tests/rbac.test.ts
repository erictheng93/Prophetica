import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ALL_ROLES,
  canAccess,
  canApprove,
  capabilityFor,
  defaultLandingFor,
  filterRoutes,
} from "../lib/rbac.js";
import type { OpsRoute } from "../lib/ops-facade.js";

const internalRoutes = [
  "/",
  "/dashboard",
  "/faq",
  "/members",
  "/points",
  "/affiliate",
  "/affiliate/commissions",
  "/affiliate/payouts",
  "/kyc-risk",
  "/compensation",
  "/approvals",
] as const;

test("admin and manager can reach every internal route", () => {
  for (const path of internalRoutes) {
    assert.equal(canAccess(path, "admin"), true, `admin -> ${path}`);
    assert.equal(canAccess(path, "manager"), true, `manager -> ${path}`);
  }
});

test("affiliate cannot reach any internal route", () => {
  for (const path of internalRoutes) {
    assert.equal(canAccess(path, "affiliate"), false, `affiliate -> ${path}`);
  }
});

test("only affiliate can reach /me/affiliate and its descendants", () => {
  for (const path of [
    "/me/affiliate",
    "/me/affiliate/points",
    "/me/affiliate/payouts",
    "/me/affiliate/sub-tree",
  ]) {
    assert.equal(canAccess(path, "affiliate"), true, `affiliate -> ${path}`);
    assert.equal(canAccess(path, "admin"), false, `admin -> ${path}`);
    assert.equal(canAccess(path, "manager"), false, `manager -> ${path}`);
  }
});

test("points BFF routes are reachable by every signed-in role", () => {
  for (const role of ALL_ROLES) {
    assert.equal(
      canAccess("/api/points/wallets/user_123/balance", role),
      true,
      `${role} -> balance BFF`,
    );
    assert.equal(
      canAccess("/api/points/manual-adjustments", role),
      true,
      `${role} -> manual adjustment BFF`,
    );
  }
});

test("FAQ catalog route is limited to admin and manager sessions", () => {
  assert.equal(canAccess("/api/faq/catalog", "admin"), true);
  assert.equal(canAccess("/api/faq/catalog", "manager"), true);
  assert.equal(canAccess("/api/faq/catalog", "affiliate"), false);
});

test("analytics BFF routes are limited to admin and manager sessions", () => {
  for (const path of [
    "/api/analytics/templates",
    "/api/analytics/dashboard-metrics",
    "/api/analytics/jobs/latest-per-template",
    "/api/analytics/jobs/rpt-job-000001/export",
  ]) {
    assert.equal(canAccess(path, "admin"), true, `admin -> ${path}`);
    assert.equal(canAccess(path, "manager"), true, `manager -> ${path}`);
    assert.equal(canAccess(path, "affiliate"), false, `affiliate -> ${path}`);
  }
});

test("admin and manager can reach /affiliate/[id] detail; affiliate cannot", () => {
  for (const path of [
    "/affiliate/AFF-001",
    "/affiliate/AFF-022",
    "/affiliate/commissions/AFF-001",
    "/affiliate/payouts/PAYOUT-001",
  ]) {
    assert.equal(canAccess(path, "admin"), true, `admin -> ${path}`);
    assert.equal(canAccess(path, "manager"), true, `manager -> ${path}`);
    assert.equal(canAccess(path, "affiliate"), false, `affiliate -> ${path}`);
  }
});

test("/me prefix does not bleed into /members or /me-anything", () => {
  // Regression: an earlier middleware build used pathname.startsWith("/me")
  // which returned true for /members. canAccess must still 403 those paths
  // for affiliate so the redirect logic can route them correctly.
  assert.equal(canAccess("/members", "affiliate"), false);
  assert.equal(canAccess("/me-something", "affiliate"), false);
  assert.equal(canAccess("/me", "affiliate"), false);
});

test("public routes are reachable by every role", () => {
  for (const role of ALL_ROLES) {
    assert.equal(canAccess("/login", role), true, `${role} -> /login`);
    assert.equal(canAccess("/403", role), true, `${role} -> /403`);
  }
});

test("canApprove only true for admin", () => {
  assert.equal(canApprove("admin", "compensation-reviews"), true);
  assert.equal(canApprove("manager", "compensation-reviews"), false);
  assert.equal(canApprove("affiliate", "compensation-reviews"), false);
});

test("manager has propose capability where admin has approve", () => {
  assert.equal(capabilityFor("admin", "points-exceptions"), "approve");
  assert.equal(capabilityFor("manager", "points-exceptions"), "propose");
  assert.equal(capabilityFor("affiliate", "points-exceptions"), undefined);
});

test("defaultLandingFor sends affiliate to /me/affiliate", () => {
  assert.equal(defaultLandingFor("admin"), "/");
  assert.equal(defaultLandingFor("manager"), "/");
  assert.equal(defaultLandingFor("affiliate"), "/me/affiliate");
});

test("filterRoutes hides every internal domain from affiliate", () => {
  const routes: OpsRoute[] = [
    {
      href: "/members",
      label: "M",
      domain: "member-management",
      summary: "",
      scope: "",
      owner: "",
      openItems: 0,
      tone: "neutral",
    },
    {
      href: "/kyc-risk",
      label: "K",
      domain: "kyc-risk-events",
      summary: "",
      scope: "",
      owner: "",
      openItems: 0,
      tone: "neutral",
    },
  ];

  assert.equal(filterRoutes(routes, "admin").length, 2);
  assert.equal(filterRoutes(routes, "manager").length, 2);
  assert.equal(filterRoutes(routes, "affiliate").length, 0);
});
