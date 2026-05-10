import "server-only";
import type { Role } from "./rbac";

/*
 * Phase 2 mock — three hardcoded users so admin/manager/affiliate flows can
 * be exercised end-to-end before user-service grows real auth endpoints.
 *
 * Replaced by a proxy to user-service in Phase 3 (see TODO.md). Until then,
 * passwords are intentionally identical and obvious so they cannot be
 * mistaken for production credentials.
 */

export interface MockUser {
  email: string;
  password: string;
  sub: string;
  role: Role;
  affiliateId?: string;
}

export const mockUsers: readonly MockUser[] = [
  { email: "admin@prophetica.local", password: "dev", sub: "U-ADMIN-001", role: "admin" },
  { email: "manager@prophetica.local", password: "dev", sub: "U-MGR-001", role: "manager" },
  {
    email: "affiliate@prophetica.local",
    password: "dev",
    sub: "U-AFF-001",
    role: "affiliate",
    affiliateId: "AFF-001",
  },
];

export function findMockUser(email: string, password: string): MockUser | undefined {
  return mockUsers.find(
    (user) => user.email.toLowerCase() === email.trim().toLowerCase() && user.password === password,
  );
}
