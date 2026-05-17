import type { OpsDomainId, OpsRoute } from "./ops-facade.js";

export const ALL_ROLES = ["admin", "manager", "affiliate"] as const;
export type Role = (typeof ALL_ROLES)[number];

export function isRole(value: unknown): value is Role {
  return (
    typeof value === "string" &&
    (ALL_ROLES as readonly string[]).includes(value)
  );
}

/*
 * SPEC-017 §6.3.1 — single source of truth for which roles can reach which
 * route. middleware, AdminNav, RoleGate, and any API route guard all read
 * from this table; never duplicate the allowlist anywhere else.
 *
 * "isPrefix" means any path that *starts with* the key is also allowed —
 * used for /me/affiliate to also cover /me/affiliate/payouts etc.
 */
type RouteRule = {
  path: string;
  isPrefix?: boolean;
  allow: readonly Role[];
};

const routeRules: readonly RouteRule[] = [
  { path: "/login", allow: ALL_ROLES },
  { path: "/403", allow: ALL_ROLES },
  { path: "/api/auth/passkeys", isPrefix: true, allow: ALL_ROLES },
  { path: "/api/points/wallets", isPrefix: true, allow: ALL_ROLES },
  { path: "/api/points/manual-adjustments", allow: ALL_ROLES },
  { path: "/api/members", isPrefix: true, allow: ALL_ROLES },
  { path: "/api/analytics", isPrefix: true, allow: ["admin", "manager"] },
  { path: "/api/faq/catalog", allow: ["admin", "manager"] },
  { path: "/", allow: ["admin", "manager"] },
  { path: "/dashboard", allow: ["admin", "manager"] },
  { path: "/faq", allow: ["admin", "manager"] },
  { path: "/members", allow: ALL_ROLES },
  { path: "/members/kyc-review", isPrefix: true, allow: ["admin", "manager"] },
  { path: "/points", allow: ["admin", "manager"] },
  { path: "/affiliate", isPrefix: true, allow: ["admin", "manager"] },
  { path: "/analytics", isPrefix: true, allow: ["admin", "manager"] },
  { path: "/kyc-risk", allow: ["admin", "manager"] },
  { path: "/compensation", allow: ["admin", "manager"] },
  { path: "/approvals", allow: ["admin", "manager"] },
  { path: "/me/affiliate", isPrefix: true, allow: ["affiliate"] },
];

export function canAccess(pathname: string, role: Role): boolean {
  const exact = routeRules.find(
    (rule) => !rule.isPrefix && rule.path === pathname,
  );
  if (exact) {
    return exact.allow.includes(role);
  }

  const prefix = routeRules.find(
    (rule) =>
      rule.isPrefix &&
      (pathname === rule.path || pathname.startsWith(`${rule.path}/`)),
  );
  if (prefix) {
    return prefix.allow.includes(role);
  }

  return false;
}

export function defaultLandingFor(role: Role): string {
  return role === "affiliate" ? "/me/affiliate" : "/";
}

/*
 * Operational domains — admin can finalize, manager can only propose / first-pass.
 * Used by RouteEntryCard, RoleGate, and DomainScope to swap CTA copy.
 */
type DomainCapability = "approve" | "propose" | "view";

const domainCapabilityByRole: Record<
  Role,
  Partial<Record<OpsDomainId, DomainCapability>>
> = {
  admin: {
    "member-management": "approve",
    "points-exceptions": "approve",
    "affiliate-review": "approve",
    "kyc-risk-events": "approve",
    "compensation-reviews": "approve",
    "manual-approvals": "approve",
  },
  manager: {
    "member-management": "view",
    "points-exceptions": "propose",
    "affiliate-review": "view",
    "kyc-risk-events": "view",
    "compensation-reviews": "propose",
    "manual-approvals": "propose",
  },
  affiliate: {
    "member-management": "propose",
  },
};

export function capabilityFor(
  role: Role,
  domain: OpsDomainId,
): DomainCapability | undefined {
  return domainCapabilityByRole[role][domain];
}

export function canApprove(role: Role, domain: OpsDomainId): boolean {
  return capabilityFor(role, domain) === "approve";
}

export function filterRoutes(
  routes: readonly OpsRoute[],
  role: Role,
): OpsRoute[] {
  return routes.filter((route) => canAccess(route.href, role));
}
