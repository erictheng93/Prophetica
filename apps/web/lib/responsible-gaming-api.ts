import type {
  ClientOptions,
  ResponsibleGamingClient,
  SelfExclusionPeriod,
  SubmitSelfExclusionRequest,
} from "@prophetica/ts-sdk";

import { tracedFetch } from "./observability/trace.js";
import { generateIdempotencyKey } from "./idempotency.js";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_DEV_USER_ID = "user_01";
const POINTS_PER_TWD = 100;
const COOLDOWN_PATH = "/v1/me/responsible-gaming/cooldown";
const SELF_EXCLUSION_PATH = "/v1/responsible-gaming/self-exclusion";

export const COOLDOWN_DURATIONS = ["24h", "7d", "30d"] as const;

export type CooldownDuration = (typeof COOLDOWN_DURATIONS)[number];

export type ResponsibleGamingSource =
  | "api-gateway"
  | "fallback:api-gateway-unavailable";

export interface SelfExclusionRecordView {
  recordId: string;
  userId: string;
  mode: "timeout" | "permanent";
  period: SelfExclusionPeriod | null;
  pointsDisposition: "hold" | "donate" | null;
  reason: string | null;
  startsAtEpochSeconds: number;
  endsAtEpochSeconds: number | null;
  frozenPoints: number;
}

export interface SelfExclusionStatusView {
  userId: string;
  active: boolean;
  record: SelfExclusionRecordView | null;
  source: ResponsibleGamingSource;
}

export interface SelfExclusionSubmissionView {
  record: SelfExclusionRecordView;
  ledgerIdempotencyKey: string;
  replayed: boolean;
  source: ResponsibleGamingSource;
}

export interface CooldownStatusView {
  userId: string;
  active: boolean;
  duration: CooldownDuration | null;
  startedAtEpochSeconds: number | null;
  endsAtEpochSeconds: number | null;
  remainingSeconds: number;
  availableDurations: CooldownDuration[];
  source: ResponsibleGamingSource;
}

export type CooldownStatus = CooldownStatusView;

export interface StartCooldownInput {
  duration: CooldownDuration;
  consent: boolean;
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;
export type ResponsibleGamingClientFactory = (
  options: ClientOptions,
) => ResponsibleGamingClient;

export interface FetchSelfExclusionStatusOptions {
  baseUrl?: string;
  userId?: string;
  fetcher?: Fetcher;
  responsibleGamingClientFactory?: ResponsibleGamingClientFactory;
}

export interface RequestSelfExclusionOptions
  extends
    FetchSelfExclusionStatusOptions,
    Omit<SubmitSelfExclusionRequest, "idempotencyKey"> {
  idempotencyKey?: string;
}

interface ApiCooldownStatus {
  user_id: string;
  active: boolean;
  duration: string | null;
  started_at_epoch_seconds: number | null;
  ends_at_epoch_seconds: number | null;
  remaining_seconds: number;
  available_durations: string[];
}

interface ApiSelfExclusionRecord {
  record_id: string;
  user_id: string;
  mode: "timeout" | "permanent";
  period: SelfExclusionPeriod | null;
  points_disposition: "hold" | "donate" | null;
  reason: string | null;
  starts_at_epoch_seconds: number;
  ends_at_epoch_seconds: number | null;
  frozen_minor_units: number;
}

interface ApiSelfExclusionStatus {
  user_id: string;
  active: boolean;
  record: ApiSelfExclusionRecord | null;
}

interface ApiSubmitSelfExclusionResponse {
  record: ApiSelfExclusionRecord;
  ledger_idempotency_key: string;
  replayed: boolean;
}

export class ResponsibleGamingApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ResponsibleGamingApiError";
    this.status = status;
    this.code = code;
  }
}

export async function fetchCooldownStatus(
  options: FetchSelfExclusionStatusOptions = {},
): Promise<CooldownStatusView> {
  const { baseUrl, userId, fetcher } = transportContext(options);

  try {
    const response = await fetcher(`${baseUrl}${COOLDOWN_PATH}`, {
      cache: "no-store",
      headers: requestHeaders(userId),
    });
    if (!response.ok) {
      return fallbackCooldownStatus(userId);
    }

    return mapCooldownStatus(
      (await response.json()) as ApiCooldownStatus,
      "api-gateway",
    );
  } catch {
    return fallbackCooldownStatus(userId);
  }
}

export async function startCooldown(
  input: StartCooldownInput,
  options: FetchSelfExclusionStatusOptions = {},
): Promise<CooldownStatusView> {
  const { baseUrl, userId, fetcher } = transportContext(options);
  const response = await fetcher(`${baseUrl}${COOLDOWN_PATH}`, {
    method: "POST",
    cache: "no-store",
    headers: {
      ...requestHeaders(userId),
      "content-type": "application/json",
    },
    body: JSON.stringify(input),
  });

  if (!response.ok) {
    const error = await decodeError(response);
    throw new ResponsibleGamingApiError(
      response.status,
      error.code,
      error.message,
    );
  }

  return mapCooldownStatus(
    (await response.json()) as ApiCooldownStatus,
    "api-gateway",
  );
}

export async function fetchSelfExclusionStatus(
  options: FetchSelfExclusionStatusOptions = {},
): Promise<SelfExclusionStatusView> {
  const { baseUrl, userId, fetcher } = transportContext(options);

  try {
    if (options.responsibleGamingClientFactory) {
      const { client } = await clientContext(options);
      const status = await client.getSelfExclusionStatus();
      return {
        userId: status.userId,
        active: status.active,
        record: status.record ? mapRecord(status.record) : null,
        source: "api-gateway",
      };
    }

    const response = await fetcher(`${baseUrl}${SELF_EXCLUSION_PATH}`, {
      cache: "no-store",
      headers: requestHeaders(userId),
    });
    if (!response.ok) {
      return fallbackStatus(userId);
    }

    const status = (await response.json()) as ApiSelfExclusionStatus;
    return {
      userId: status.user_id,
      active: status.active,
      record: status.record ? mapApiRecord(status.record) : null,
      source: "api-gateway",
    };
  } catch {
    return fallbackStatus(userId);
  }
}

export async function requestSelfExclusion(
  options: RequestSelfExclusionOptions,
): Promise<
  SelfExclusionSubmissionView | { source: "fallback:api-gateway-unavailable" }
> {
  const { baseUrl, userId, fetcher } = transportContext(options);

  try {
    const request = selfExclusionRequest(options);

    if (options.responsibleGamingClientFactory) {
      const { client } = await clientContext(options);
      const response = await client.submitSelfExclusion(request);
      return {
        record: mapRecord(response.record),
        ledgerIdempotencyKey: response.ledgerIdempotencyKey,
        replayed: response.replayed,
        source: "api-gateway",
      };
    }

    const response = await fetcher(`${baseUrl}${SELF_EXCLUSION_PATH}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        ...requestHeaders(userId),
        "content-type": "application/json",
        "idempotency-key": options.idempotencyKey ?? generateIdempotencyKey(),
      },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      return { source: "fallback:api-gateway-unavailable" };
    }

    const payload = (await response.json()) as ApiSubmitSelfExclusionResponse;
    return {
      record: mapApiRecord(payload.record),
      ledgerIdempotencyKey: payload.ledger_idempotency_key,
      replayed: payload.replayed,
      source: "api-gateway",
    };
  } catch {
    return { source: "fallback:api-gateway-unavailable" };
  }
}

async function clientContext(options: FetchSelfExclusionStatusOptions) {
  const { baseUrl, userId, fetcher } = transportContext(options);
  const factory =
    options.responsibleGamingClientFactory ??
    (await loadResponsibleGamingClientFactory());
  const client = factory({
    baseUrl,
    auth: { userId },
    fetcher,
  });

  return { userId, client };
}

function transportContext(options: FetchSelfExclusionStatusOptions) {
  const baseUrl = trimTrailingSlash(
    options.baseUrl ??
      process.env.API_GATEWAY_BASE_URL ??
      DEFAULT_API_GATEWAY_BASE_URL,
  );
  const userId =
    options.userId ?? process.env.PROPHETICA_DEV_USER_ID ?? DEFAULT_DEV_USER_ID;
  const fetcher =
    options.fetcher ?? ((input, init) => tracedFetch(input, init));
  return { baseUrl, userId, fetcher };
}

async function loadResponsibleGamingClientFactory(): Promise<ResponsibleGamingClientFactory> {
  const { createResponsibleGamingClient } = await import("@prophetica/ts-sdk");
  return createResponsibleGamingClient;
}

function mapRecord(
  record: import("@prophetica/ts-sdk").SelfExclusionRecord,
): SelfExclusionRecordView {
  return {
    recordId: record.recordId,
    userId: record.userId,
    mode: record.mode,
    period: record.period,
    pointsDisposition: record.pointsDisposition,
    reason: record.reason,
    startsAtEpochSeconds: record.startsAtEpochSeconds,
    endsAtEpochSeconds: record.endsAtEpochSeconds,
    frozenPoints: minorUnitsToPoints(record.frozenMinorUnits),
  };
}

function mapApiRecord(record: ApiSelfExclusionRecord): SelfExclusionRecordView {
  return {
    recordId: record.record_id,
    userId: record.user_id,
    mode: record.mode,
    period: record.period,
    pointsDisposition: record.points_disposition,
    reason: record.reason,
    startsAtEpochSeconds: record.starts_at_epoch_seconds,
    endsAtEpochSeconds: record.ends_at_epoch_seconds,
    frozenPoints: minorUnitsToPoints(record.frozen_minor_units),
  };
}

function selfExclusionRequest(
  options: RequestSelfExclusionOptions,
): SubmitSelfExclusionRequest {
  const request: SubmitSelfExclusionRequest = {
    mode: options.mode,
    consent: options.consent,
  };
  if (options.period !== undefined) {
    request.period = options.period;
  }
  if (options.pointsDisposition !== undefined) {
    request.pointsDisposition = options.pointsDisposition;
  }
  if (options.reason !== undefined) {
    request.reason = options.reason;
  }
  if (options.idempotencyKey !== undefined) {
    request.idempotencyKey = options.idempotencyKey;
  }
  return request;
}

function mapCooldownStatus(
  status: ApiCooldownStatus,
  source: ResponsibleGamingSource,
): CooldownStatusView {
  return {
    userId: status.user_id,
    active: status.active,
    duration: isCooldownDuration(status.duration) ? status.duration : null,
    startedAtEpochSeconds: status.started_at_epoch_seconds,
    endsAtEpochSeconds: status.ends_at_epoch_seconds,
    remainingSeconds: status.remaining_seconds,
    availableDurations: status.available_durations.filter(isCooldownDuration),
    source,
  };
}

function fallbackStatus(userId: string): SelfExclusionStatusView {
  return {
    userId,
    active: false,
    record: null,
    source: "fallback:api-gateway-unavailable",
  };
}

function fallbackCooldownStatus(userId: string): CooldownStatusView {
  return {
    userId,
    active: false,
    duration: null,
    startedAtEpochSeconds: null,
    endsAtEpochSeconds: null,
    remainingSeconds: 0,
    availableDurations: [...COOLDOWN_DURATIONS],
    source: "fallback:api-gateway-unavailable",
  };
}

function requestHeaders(userId: string): Record<string, string> {
  return {
    accept: "application/json",
    "x-user-id": userId,
  };
}

function isCooldownDuration(value: unknown): value is CooldownDuration {
  return COOLDOWN_DURATIONS.some((duration) => duration === value);
}

async function decodeError(
  response: Response,
): Promise<{ code: string; message: string }> {
  try {
    const payload = (await response.json()) as {
      error?: unknown;
      message?: unknown;
    };
    return {
      code:
        typeof payload.error === "string"
          ? payload.error
          : "responsible_gaming_error",
      message:
        typeof payload.message === "string"
          ? payload.message
          : "Responsible gaming request failed",
    };
  } catch {
    return {
      code: "responsible_gaming_error",
      message: "Responsible gaming request failed",
    };
  }
}

function minorUnitsToPoints(minorUnits: number | bigint): number {
  return Number(minorUnits) / POINTS_PER_TWD;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
