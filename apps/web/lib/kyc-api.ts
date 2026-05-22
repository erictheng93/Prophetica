import type {
  ClientOptions,
  KycClient,
  KycStatus,
  KycStatusView,
  KycTargetTier,
  KycTier,
  StartKycUpgradeResponse,
  VerifyTierOneResponse,
} from "@prophetica/ts-sdk";
import { buildKycIdempotencyKey } from "./kyc-idempotency.js";

export const KYC_FALLBACK_USER_ID = "user_01";

const DEFAULT_KYC_SERVICE_BASE_URL = "http://localhost:8085";

export type KycSubmitErrorStatus =
  | "service_unavailable"
  | "duplicate_document"
  | "invalid_submission";

export type KycClientFactory = (options: ClientOptions) => KycClient;

export interface KycOverview {
  userId: string;
  currentTier: KycTier;
  currentTierNumber: 0 | 1 | 2 | 3;
  status: KycStatus;
  provider: string | null;
  providerApplicantId: string | null;
  country: string | null;
  tier1RecordId: string | null;
  declaredMonthlyIncome: number | null;
  monthlyLimitPoints: number | null;
  submittedAt: string | null;
  reviewedAt: string | null;
  expiresAt: string | null;
  source: "kyc-service" | "fallback:kyc-service-unavailable";
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface FetchKycOverviewOptions {
  baseUrl?: string;
  userId?: string;
  fetcher?: Fetcher;
  kycClientFactory?: KycClientFactory;
}

export interface SubmitTierOneVerificationInput {
  userId?: string;
  legalName: string;
  birthday: string;
  country: string;
  documentId: string;
  consent: boolean;
}

export interface StartKycUpgradeInput {
  targetTier: KycTargetTier;
  declaredMonthlyIncome?: number | null;
  retryAttempt?: number;
}

export interface KycMutationOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
  kycClientFactory?: KycClientFactory;
}

export async function fetchKycOverview(
  options: FetchKycOverviewOptions = {},
): Promise<KycOverview> {
  const baseUrl = resolveKycServiceBaseUrl(options.baseUrl);
  const userId =
    options.userId ??
    process.env.PROPHETICA_DEV_USER_ID ??
    KYC_FALLBACK_USER_ID;
  const kycClientFactory =
    options.kycClientFactory ?? (await loadKycClientFactory());
  const kycClient = kycClientFactory({
    baseUrl,
    auth: { userId },
    fetcher: options.fetcher ?? fetch,
  });

  try {
    return mapStatus(await kycClient.getStatus(userId), "kyc-service");
  } catch {
    return mapStatus(
      {
        userId,
        currentTier: "TIER_0",
        status: "pending",
        provider: null,
        providerApplicantId: null,
        country: null,
        tier1RecordId: null,
        declaredMonthlyIncome: null,
        submittedAt: null,
        reviewedAt: null,
        expiresAt: null,
      },
      "fallback:kyc-service-unavailable",
    );
  }
}

export async function submitTierOneVerification(
  input: SubmitTierOneVerificationInput,
  options: KycMutationOptions = {},
): Promise<VerifyTierOneResponse> {
  const { userId, kycClient } = await mutationContext(options);
  return kycClient.verifyTierOne({
    userId,
    legalName: input.legalName,
    birthday: input.birthday,
    country: input.country,
    documentId: input.documentId,
    consent: input.consent,
  });
}

export async function startKycUpgrade(
  input: StartKycUpgradeInput,
  options: KycMutationOptions = {},
): Promise<StartKycUpgradeResponse> {
  const { userId, kycClient } = await mutationContext(options);
  let tier1RecordId: string | null = null;
  try {
    tier1RecordId = (await kycClient.getStatus(userId)).tier1RecordId;
  } catch {
    tier1RecordId = null;
  }
  return kycClient.startUpgrade({
    userId,
    targetTier: input.targetTier,
    declaredMonthlyIncome: input.declaredMonthlyIncome,
    idempotencyKey: buildKycIdempotencyKey({
      userId,
      targetTier: input.targetTier,
      tier1RecordId,
      retryAttempt: input.retryAttempt,
    }),
  });
}

export function resolveKycUserId(userIdOption?: string): string {
  return (
    userIdOption ?? process.env.PROPHETICA_DEV_USER_ID ?? KYC_FALLBACK_USER_ID
  );
}

export function resolveKycServiceBaseUrl(baseUrlOption?: string): string {
  return trimTrailingSlash(
    firstNonBlank(
      baseUrlOption,
      process.env.KYC_SERVICE_BASE_URL,
      process.env.KYC_SERVICE_URL,
      DEFAULT_KYC_SERVICE_BASE_URL,
    ),
  );
}

export function resolveKycSubmitErrorStatus(
  error: unknown,
): KycSubmitErrorStatus {
  const apiError = readApiError(error);
  if (apiError) {
    if (apiError.code === "duplicate_document") {
      return "duplicate_document";
    }
    if (
      apiError.code === "validation_failed" ||
      apiError.status === 400 ||
      apiError.status === 422
    ) {
      return "invalid_submission";
    }
  }

  return "service_unavailable";
}

export function tierMonthlyLimitPoints(tier: KycTier): number | null {
  switch (tier) {
    case "TIER_0":
      return 0;
    case "TIER_1":
      return 30_000;
    case "TIER_2":
      return 300_000;
    case "TIER_3":
      return null;
  }
}

async function mutationContext(options: KycMutationOptions) {
  const baseUrl = resolveKycServiceBaseUrl(options.baseUrl);
  const userId = resolveKycUserId();
  const kycClientFactory =
    options.kycClientFactory ?? (await loadKycClientFactory());

  return {
    userId,
    kycClient: kycClientFactory({
      baseUrl,
      auth: { userId },
      fetcher: options.fetcher ?? fetch,
    }),
  };
}

function mapStatus(
  status: KycStatusView,
  source: KycOverview["source"],
): KycOverview {
  return {
    ...status,
    currentTierNumber: tierNumber(status.currentTier),
    monthlyLimitPoints: tierMonthlyLimitPoints(status.currentTier),
    source,
  };
}

function tierNumber(tier: KycTier): 0 | 1 | 2 | 3 {
  switch (tier) {
    case "TIER_0":
      return 0;
    case "TIER_1":
      return 1;
    case "TIER_2":
      return 2;
    case "TIER_3":
      return 3;
  }
}

async function loadKycClientFactory(): Promise<KycClientFactory> {
  const { createKycClient } = await import("@prophetica/ts-sdk");
  return createKycClient;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function firstNonBlank(...values: Array<string | undefined>): string {
  return (
    values.find((value) => value !== undefined && value.trim() !== "") ?? ""
  );
}

function readApiError(
  error: unknown,
): { code?: string; status?: number } | null {
  if (!error || typeof error !== "object") {
    return null;
  }
  const record = error as Record<string, unknown>;
  const envelope = record.envelope;
  if (!envelope || typeof envelope !== "object") {
    return null;
  }
  const envelopeRecord = envelope as Record<string, unknown>;
  return {
    code:
      typeof envelopeRecord.code === "string" ? envelopeRecord.code : undefined,
    status: typeof record.status === "number" ? record.status : undefined,
  };
}
