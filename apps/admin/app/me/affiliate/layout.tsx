import type { ReactNode } from "react";
import { AffiliateShell } from "@/components/affiliate-shell";
import { RoleGate } from "@/components/role-gate";
import { fetchAffiliateDashboard } from "@/lib/affiliate-management-bff";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AffiliatePortalLayout({
  children,
}: {
  children: ReactNode;
}) {
  return (
    <RoleGate allow={["affiliate"]}>
      {async (viewer) => {
        const dashboard = await fetchAffiliateDashboard(
          viewer,
          await getUpstreamSessionToken(),
        );

        return (
          <AffiliateShell
            affiliateId={viewer.affiliateId ?? dashboard?.referral_code ?? "—"}
          >
            {children}
          </AffiliateShell>
        );
      }}
    </RoleGate>
  );
}
