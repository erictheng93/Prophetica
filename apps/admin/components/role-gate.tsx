import { redirect } from "next/navigation";
import type { ReactNode } from "react";
import { defaultLandingFor, type Role } from "@/lib/rbac";
import { getViewer, type SessionPayload } from "@/lib/session";

interface RoleGateProps {
  allow: readonly Role[];
  children: (viewer: SessionPayload) => ReactNode;
}

/*
 * Server-side RBAC gate. middleware already redirects unauthenticated and
 * unauthorized requests, but pages still call this to (a) get a typed
 * viewer object without each page re-reading the cookie, and (b) be safe
 * if middleware is ever bypassed (e.g. internal RSC fetch).
 */
export async function RoleGate({ allow, children }: RoleGateProps) {
  const viewer = await getViewer();
  if (!viewer) {
    redirect("/login");
  }
  if (!allow.includes(viewer.role)) {
    redirect(viewer.role === "affiliate" ? defaultLandingFor("affiliate") : "/403");
  }
  return <>{children(viewer)}</>;
}
