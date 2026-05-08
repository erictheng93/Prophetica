export type KycProviderName = "sumsub" | "onfido" | "jumio" | "mock";

export type KycTargetTier = "TIER_2" | "TIER_3";

export interface KycSessionStartParams {
  sdkToken: string;
  targetTier: KycTargetTier;
  locale: string;
  country: string;
}

export type KycStep =
  | "consent"
  | "document"
  | "selfie"
  | "liveness"
  | "address"
  | "review";

export type KycInlineOutcome = "approved" | "rejected" | "pending_review";

export type KycErrorCode =
  | "sdk_load_failed"
  | "sdk_token_expired"
  | "browser_unsupported"
  | "permission_denied"
  | "network_lost"
  | "unknown";

export type KycSessionEvent =
  | { kind: "ready" }
  | { kind: "step_changed"; step: KycStep }
  | { kind: "submitted" }
  | { kind: "completed"; outcome: KycInlineOutcome }
  | { kind: "abandoned" }
  | { kind: "error"; code: KycErrorCode; recoverable: boolean };

export type KycEventListener = (event: KycSessionEvent) => void;

export type KycEventUnsubscribe = () => void;

export interface KycSessionHandle {
  unmount(): void;
  subscribe(listener: KycEventListener): KycEventUnsubscribe;
}

export interface KycIdentityProvider {
  readonly name: KycProviderName;
  mount(
    target: HTMLElement,
    params: KycSessionStartParams,
  ): Promise<KycSessionHandle>;
}

export type KycSdkErrorCode =
  | KycErrorCode
  | "unknown_sdk_token_prefix"
  | "not_implemented";

export class KycSdkError extends Error {
  readonly code: KycSdkErrorCode;

  constructor(code: KycSdkErrorCode, message?: string) {
    super(message ?? code);
    this.name = "KycSdkError";
    this.code = code;
  }
}
