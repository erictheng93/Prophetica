import type { Role } from "./rbac.js";

export function toAdminRole(value: unknown): Role | null {
  return value === "admin" || value === "manager" || value === "affiliate" ? value : null;
}
