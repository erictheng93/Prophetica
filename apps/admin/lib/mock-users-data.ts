import type { Role } from "./rbac.js";

export interface MockUser {
  email: string;
  password: string;
  sub: string;
  role: Role;
  affiliateId?: string;
}

export const mockUsers: readonly MockUser[] = [
  { email: "admin@prophetica.local", password: "admin", sub: "U-ADMIN-001", role: "admin" },
  { email: "manager@prophetica.local", password: "manager", sub: "U-MGR-001", role: "manager" },
  { email: "manager2@prophetica.local", password: "manager2", sub: "U-MGR-002", role: "manager" },
  {
    email: "affiliate@prophetica.local",
    password: "affiliate",
    sub: "U-AFF-001",
    role: "affiliate",
    affiliateId: "AFF-001",
  },
  {
    email: "affiliate2@prophetica.local",
    password: "affiliate2",
    sub: "U-AFF-002",
    role: "affiliate",
    affiliateId: "AFF-014",
  },
];

export function findMockUser(email: string, password: string): MockUser | undefined {
  return mockUsers.find(
    (user) => user.email.toLowerCase() === email.trim().toLowerCase() && user.password === password,
  );
}
