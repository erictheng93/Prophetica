import { adminCopy } from "@/lib/admin-copy";
import type { AffiliateStatus } from "@/lib/affiliates";
import { StatusPill } from "./status-pill";
import type { OpsTone } from "@/lib/ops-facade";

const toneByStatus: Record<AffiliateStatus, OpsTone> = {
  active: "success",
  review: "warning",
  frozen: "critical",
  onboarding: "neutral",
  archived: "neutral",
};

const labelByStatus: Record<AffiliateStatus, string> = {
  active: adminCopy.affiliateAdmin.statusActive,
  review: adminCopy.affiliateAdmin.statusReview,
  frozen: adminCopy.affiliateAdmin.statusFrozen,
  onboarding: adminCopy.affiliateAdmin.statusOnboarding,
  archived: "已封存",
};

interface AffiliateStatusPillProps {
  status: AffiliateStatus;
}

export function AffiliateStatusPill({ status }: AffiliateStatusPillProps) {
  return (
    <StatusPill tone={toneByStatus[status]}>{labelByStatus[status]}</StatusPill>
  );
}
