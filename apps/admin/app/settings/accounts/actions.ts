"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { updateAdminAffiliate } from "@/lib/affiliate-management-bff";
import {
  createManagerAccount,
  disableManagerAccount,
  updateManagerAccount,
  type ManagerAccountMutationPayload,
} from "@/lib/manager-accounts-bff";
import { getViewer } from "@/lib/session";

export async function createManagerAccountAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const response = await createManagerAccount(viewer, mutationPayloadFromForm(formData));
  if (!response?.ok) redirect("/settings/accounts?error=manager-create-failed");
  revalidatePath("/settings/accounts");
  redirect("/settings/accounts?status=manager-created");
}

export async function updateManagerAccountAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const userId = requiredText(formData, "userId");
  const response = await updateManagerAccount(
    viewer,
    userId,
    mutationPayloadFromForm(formData),
  );
  if (!response?.ok) redirect("/settings/accounts?error=manager-update-failed");
  revalidatePath("/settings/accounts");
  redirect("/settings/accounts?status=manager-updated");
}

export async function disableManagerAccountAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const userId = requiredText(formData, "userId");
  const response = await disableManagerAccount(viewer, userId);
  if (!response?.ok) {
    const error =
      response?.status === 409
        ? "manager-has-affiliates"
        : "manager-disable-failed";
    redirect(`/settings/accounts?error=${error}`);
  }
  revalidatePath("/settings/accounts");
  redirect("/settings/accounts?status=manager-disabled");
}

export async function reassignAffiliateManagerAction(
  formData: FormData,
): Promise<void> {
  const viewer = await requireAdminViewer();
  const affiliateId = requiredText(formData, "affiliateId");
  const result = await updateAdminAffiliate(viewer, affiliateId, {
    managingManagerId: requiredText(formData, "managingManagerId"),
    reason: requiredText(formData, "reason"),
    authorizationBasis: requiredText(formData, "authorizationBasis"),
  });
  if (!result) redirect("/settings/accounts?error=affiliate-reassign-failed");
  revalidatePath("/settings/accounts");
  revalidatePath("/affiliate");
  revalidatePath(`/affiliate/${affiliateId}`);
  redirect("/settings/accounts?status=affiliate-reassigned");
}

async function requireAdminViewer() {
  const viewer = await getViewer();
  if (!viewer || viewer.role !== "admin") redirect("/403");
  return viewer;
}

function mutationPayloadFromForm(formData: FormData): ManagerAccountMutationPayload {
  const payload: ManagerAccountMutationPayload = {
    reason: requiredText(formData, "reason"),
    authorizationBasis: requiredText(formData, "authorizationBasis"),
  };
  assignText(formData, payload, "userId");
  assignText(formData, payload, "displayName");
  assignText(formData, payload, "email");
  assignText(formData, payload, "phone");
  assignText(formData, payload, "password");
  assignText(formData, payload, "country");
  assignText(formData, payload, "timezone");
  assignText(formData, payload, "tier");
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
  payload: ManagerAccountMutationPayload,
  key:
    | "userId"
    | "displayName"
    | "email"
    | "phone"
    | "password"
    | "country"
    | "timezone"
    | "tier",
): void {
  const value = optionalText(formData, key);
  if (value) payload[key] = value;
}
