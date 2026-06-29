"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import {
  archiveAdminAffiliate,
  createAdminAffiliate,
  reviewAffiliateCommissionAdjustment,
  reviewAffiliatePayout,
  updateAdminAffiliate,
} from "@/lib/affiliate-management-bff";
import type {
  AffiliateCreditAccountStatus,
  AffiliateCreditRating,
  AffiliateMutationPayload,
  AffiliateStatus,
} from "@/lib/affiliates";
import { getViewer } from "@/lib/session";

export async function createAffiliateAction(formData: FormData): Promise<void> {
  const viewer = await requireAdminViewer();
  const payload = mutationPayloadFromForm(formData);
  const result = await createAdminAffiliate(viewer, payload);
  if (!result) redirect("/affiliate?error=affiliate-create-failed");
  if (result.status === "repair_required") {
    redirect(
      `/affiliate?error=affiliate-create-repair-required&affiliateAccountSaga=${encodeURIComponent(result.saga.idempotencyKey)}`,
    );
  }
  revalidatePath("/affiliate");
  redirect(`/affiliate/${encodeURIComponent(result.affiliate.affiliate.id)}`);
}

export async function updateAffiliateAction(formData: FormData): Promise<void> {
  const viewer = await requireAdminViewer();
  const affiliateId = requiredText(formData, "affiliateId");
  const result = await updateAdminAffiliate(
    viewer,
    affiliateId,
    mutationPayloadFromForm(formData),
  );
  if (!result)
    redirect(
      `/affiliate/${encodeURIComponent(affiliateId)}?error=affiliate-update-failed`,
    );
  revalidatePath("/affiliate");
  revalidatePath(`/affiliate/${affiliateId}`);
}

export async function archiveAffiliateAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const affiliateId = requiredText(formData, "affiliateId");
  const result = await archiveAdminAffiliate(viewer, affiliateId, {
    reason: requiredText(formData, "reason"),
    authorizationBasis: requiredText(formData, "authorizationBasis"),
  });
  if (!result)
    redirect(
      `/affiliate/${encodeURIComponent(affiliateId)}?error=affiliate-archive-failed`,
    );
  revalidatePath("/affiliate");
  redirect("/affiliate");
}

export async function reviewCommissionAdjustmentAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const reviewId = requiredText(formData, "reviewId");
  const result = await reviewAffiliateCommissionAdjustment(viewer, reviewId, {
    decision: reviewDecision(formData),
    reason: requiredText(formData, "reason"),
  });
  if (!result) redirect("/affiliate/commissions?error=review-failed");
  revalidatePath("/affiliate/commissions");
}

export async function reviewPayoutAction(formData: FormData): Promise<void> {
  const viewer = await requireAdminViewer();
  const reviewId = requiredText(formData, "reviewId");
  const result = await reviewAffiliatePayout(viewer, reviewId, {
    decision: reviewDecision(formData),
    reason: requiredText(formData, "reason"),
  });
  if (!result) redirect("/affiliate/payouts?error=review-failed");
  revalidatePath("/affiliate/payouts");
}

async function requireAdminViewer() {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    redirect("/403");
  }
  return viewer;
}

function reviewDecision(formData: FormData): "approved" | "rejected" {
  const decision = requiredText(formData, "decision");
  if (decision !== "approved" && decision !== "rejected") {
    throw new Error("decision is invalid");
  }
  return decision;
}

function mutationPayloadFromForm(formData: FormData): AffiliateMutationPayload {
  const payload: AffiliateMutationPayload = {
    reason: requiredText(formData, "reason"),
    authorizationBasis: requiredText(formData, "authorizationBasis"),
  };
  assignText(formData, payload, "displayName");
  assignText(formData, payload, "userId");
  assignText(formData, payload, "affiliateId");
  assignText(formData, payload, "email");
  assignText(formData, payload, "phone");
  assignText(formData, payload, "password");
  assignText(formData, payload, "ownerSponsorId");
  assignText(formData, payload, "managingManagerId");
  assignText(formData, payload, "effectiveAt");
  assignText(formData, payload, "expiresAt");
  assignText(formData, payload, "creditReviewExpiresAt");
  const status = optionalText(formData, "status");
  if (status) payload.status = status as AffiliateStatus;
  const rating = optionalText(formData, "creditRating");
  if (rating) payload.creditRating = rating as AffiliateCreditRating;
  const creditStatus = optionalText(formData, "creditStatus");
  if (creditStatus)
    payload.creditStatus = creditStatus as AffiliateCreditAccountStatus;
  assignNumber(formData, payload, "commissionRateBps");
  assignTier(formData, payload);
  assignNumber(formData, payload, "maxActiveCreditAccounts");
  assignNumber(formData, payload, "totalCreditLimitMinorUnits");
  assignNumber(formData, payload, "perAccountLimitMinorUnits");
  return payload;
}

function requiredText(formData: FormData, key: string): string {
  const value = optionalText(formData, key);
  if (!value) throw new Error(`${key} is required`);
  return value;
}

function optionalText(formData: FormData, key: string): string | undefined {
  const value = formData.get(key);
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function assignText(
  formData: FormData,
  payload: AffiliateMutationPayload,
  key:
    | "displayName"
    | "userId"
    | "affiliateId"
    | "email"
    | "phone"
    | "password"
    | "ownerSponsorId"
    | "managingManagerId"
    | "effectiveAt"
    | "expiresAt"
    | "creditReviewExpiresAt",
): void {
  const value = optionalText(formData, key);
  if (value) payload[key] = value;
}

function assignNumber(
  formData: FormData,
  payload: AffiliateMutationPayload,
  key:
    | "commissionRateBps"
    | "maxActiveCreditAccounts"
    | "totalCreditLimitMinorUnits"
    | "perAccountLimitMinorUnits",
): void {
  const value = optionalText(formData, key);
  if (!value) return;
  const parsed = Number(value);
  if (Number.isFinite(parsed)) payload[key] = parsed;
}

function assignTier(
  formData: FormData,
  payload: AffiliateMutationPayload,
): void {
  const value = optionalText(formData, "tier");
  if (!value) return;
  const parsed = Number(value);
  if (parsed === 1 || parsed === 2 || parsed === 3) payload.tier = parsed;
}
