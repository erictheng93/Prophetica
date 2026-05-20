// SPEC-023 §11 D-3 / SPEC-014 §6.1.2.1 transitional path.
//
// Local dev needs to call the api-gateway with `x-user-id` + the transition
// caller header before all BFF callers are migrated to Bearer auth. That dev
// fallback MUST NOT activate in staging / preview / production. Keying off
// `NODE_ENV !== "production"` is a negative gate that opens an impersonation
// surface in any environment that forgets to set `NODE_ENV=production`.
//
// This module is the single source of truth: callers check
// `devBffFallbackEnabled()`. Activation requires an explicit positive opt-in.

export const DEV_BFF_FALLBACK_ENV = "PROPHETICA_ALLOW_DEV_BFF";

export function devBffFallbackEnabled(): boolean {
  return process.env[DEV_BFF_FALLBACK_ENV] === "1";
}
