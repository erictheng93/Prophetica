import { HttpClient, type ClientOptions } from "../runtime/fetch.js";
import { generateUuidV7 } from "../runtime/idempotency.js";
import type {
  KycStatusView,
  StartKycUpgradeRequest,
  StartKycUpgradeResponse,
  VerifyTierOneRequest,
  VerifyTierOneResponse,
} from "./types.js";

interface ApiKycStatusResponse {
  user_id: string;
  current_tier: KycStatusView["currentTier"];
  status: KycStatusView["status"];
  provider: string | null;
  provider_applicant_id: string | null;
  declared_monthly_income: number | null;
  submitted_at: string | null;
  reviewed_at: string | null;
  expires_at: string | null;
}

interface ApiVerifyTierOneResponse {
  user_id: string;
  target_tier: VerifyTierOneResponse["targetTier"];
  status: VerifyTierOneResponse["status"];
  provider: string;
  document_id_hash: string;
  replayed: boolean;
}

interface ApiStartKycUpgradeResponse {
  user_id: string;
  target_tier: StartKycUpgradeResponse["targetTier"];
  status: StartKycUpgradeResponse["status"];
  provider: string;
  provider_applicant_id: string | null;
  sdk_token: string;
  expires_at: string | null;
  replayed: boolean;
}

export interface KycClient {
  getStatus(userId: string): Promise<KycStatusView>;
  verifyTierOne(request: VerifyTierOneRequest): Promise<VerifyTierOneResponse>;
  startUpgrade(request: StartKycUpgradeRequest): Promise<StartKycUpgradeResponse>;
}

export function createKycClient(options: ClientOptions): KycClient {
  const http = new HttpClient(options);

  return {
    async getStatus(userId: string): Promise<KycStatusView> {
      return mapStatus(
        await http.json<ApiKycStatusResponse>({
          path: `/api/v1/kyc/users/${encodeURIComponent(userId)}/status`,
        }),
      );
    },

    async verifyTierOne(request: VerifyTierOneRequest): Promise<VerifyTierOneResponse> {
      const response = await http.json<ApiVerifyTierOneResponse>({
        method: "POST",
        path: `/api/v1/kyc/users/${encodeURIComponent(request.userId)}/tier-1`,
        body: {
          idempotency_key: request.idempotencyKey ?? generateUuidV7(),
          legal_name: request.legalName,
          birthday: request.birthday,
          country: request.country,
          document_id: request.documentId,
          consent: request.consent,
        },
      });

      return {
        userId: response.user_id,
        targetTier: response.target_tier,
        status: response.status,
        provider: response.provider,
        documentIdHash: response.document_id_hash,
        replayed: response.replayed,
      };
    },

    async startUpgrade(request: StartKycUpgradeRequest): Promise<StartKycUpgradeResponse> {
      const response = await http.json<ApiStartKycUpgradeResponse>({
        method: "POST",
        path: `/api/v1/kyc/users/${encodeURIComponent(request.userId)}/upgrades`,
        body: {
          target_tier: request.targetTier,
          idempotency_key: request.idempotencyKey ?? generateUuidV7(),
          declared_monthly_income: request.declaredMonthlyIncome,
        },
      });

      return {
        userId: response.user_id,
        targetTier: response.target_tier,
        status: response.status,
        provider: response.provider,
        providerApplicantId: response.provider_applicant_id,
        sdkToken: response.sdk_token,
        expiresAt: response.expires_at,
        replayed: response.replayed,
      };
    },
  };
}

function mapStatus(response: ApiKycStatusResponse): KycStatusView {
  return {
    userId: response.user_id,
    currentTier: response.current_tier,
    status: response.status,
    provider: response.provider,
    providerApplicantId: response.provider_applicant_id,
    declaredMonthlyIncome: response.declared_monthly_income,
    submittedAt: response.submitted_at,
    reviewedAt: response.reviewed_at,
    expiresAt: response.expires_at,
  };
}
