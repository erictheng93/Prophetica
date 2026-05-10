import assert from "node:assert/strict";
import { test } from "node:test";
import { ALL_ROLES, canAccess, canApprove, capabilityFor, defaultLandingFor, filterRoutes } from "../lib/rbac.js";
import type { OpsRoute } from "../lib/ops-facade.js";

const internalRoutes = ["/", "/dashboard", "/members", "/points", "/affiliate", "/kyc-risk", "/compensation", "/approvals"] as const;

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
  for (const path of ["/me/affiliate", "/me/affiliate/payouts", "/me/affiliate/sub-tree"]) {
    assert.equal(canAccess(path, "affiliate"), true, `affiliate -> ${path}`);
    assert.equal(canAccess(path, "admin"), false, `admin -> ${path}`);
    assert.equal(canAccess(path, "manager"), false, `manager -> ${path}`);
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
