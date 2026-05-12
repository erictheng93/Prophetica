import { HttpClient, type ClientOptions } from "../runtime/fetch.js";
import { generateUuidV7 } from "../runtime/idempotency.js";
import { parseI64, type JsonI64 } from "../runtime/i64.js";
import type {
  PointsDisposition,
  SelfExclusionMode,
  SelfExclusionPeriod,
  SelfExclusionRecord,
  SelfExclusionStatus,
  SubmitSelfExclusionRequest,
  SubmitSelfExclusionResponse,
} from "./types.js";

interface ApiSelfExclusionRecord {
  record_id: string;
  user_id: string;
  mode: SelfExclusionMode;
  period: SelfExclusionPeriod | null;
  points_disposition: PointsDisposition | null;
  reason: string | null;
  starts_at_epoch_seconds: number;
  ends_at_epoch_seconds: number | null;
  frozen_minor_units: JsonI64;
}

interface ApiSelfExclusionStatus {
  user_id: string;
  active: boolean;
  record: ApiSelfExclusionRecord | null;
  source: string;
}

interface ApiSubmitSelfExclusionResponse {
  record: ApiSelfExclusionRecord;
  ledger_idempotency_key: string;
  replayed: boolean;
}

export interface ResponsibleGamingClient {
  getSelfExclusionStatus(): Promise<SelfExclusionStatus>;
  submitSelfExclusion(
    request: SubmitSelfExclusionRequest,
  ): Promise<SubmitSelfExclusionResponse>;
}

export function createResponsibleGamingClient(
  options: ClientOptions,
): ResponsibleGamingClient {
  const http = new HttpClient(options);

  return {
    async getSelfExclusionStatus(): Promise<SelfExclusionStatus> {
      const response = await http.json<ApiSelfExclusionStatus>({
        path: "/v1/responsible-gaming/self-exclusion",
      });
      return mapStatus(response);
    },

    async submitSelfExclusion(
      request: SubmitSelfExclusionRequest,
    ): Promise<SubmitSelfExclusionResponse> {
      const response = await http.json<ApiSubmitSelfExclusionResponse>({
        method: "POST",
        path: "/v1/responsible-gaming/self-exclusion",
        headers: {
          "idempotency-key": request.idempotencyKey ?? generateUuidV7(),
        },
        body: {
          mode: request.mode,
          period: request.period,
          points_disposition: request.pointsDisposition,
          consent: request.consent,
          reason: request.reason,
        },
      });

      return {
        record: mapRecord(response.record),
        ledgerIdempotencyKey: response.ledger_idempotency_key,
        replayed: response.replayed,
      };
    },
  };
}

function mapStatus(status: ApiSelfExclusionStatus): SelfExclusionStatus {
  return {
    userId: status.user_id,
    active: status.active,
    record: status.record ? mapRecord(status.record) : null,
    source: status.source,
  };
}

function mapRecord(record: ApiSelfExclusionRecord): SelfExclusionRecord {
  return {
    recordId: record.record_id,
    userId: record.user_id,
    mode: record.mode,
    period: record.period,
    pointsDisposition: record.points_disposition,
    reason: record.reason,
    startsAtEpochSeconds: record.starts_at_epoch_seconds,
    endsAtEpochSeconds: record.ends_at_epoch_seconds,
    frozenMinorUnits: parseI64(record.frozen_minor_units),
  };
}
