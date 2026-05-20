// SPEC-023 plan v0.1.3 follow-up S-2 (and same gating-vector class as H-1).
//
// Cookie `Secure` was conditional on `process.env.NODE_ENV === "production"`.
// That is a negative gate: any environment that forgets to set
// NODE_ENV=production drops the `Secure` attribute, even when the page is
// served over HTTPS. Downgraded request paths (mixed-content tabs, cached
// MITM scenarios on staging) could then leak the session cookie over HTTP.
//
// Default-on-secure is the safe baseline. Local dev opts out via
// `PROPHETICA_INSECURE_COOKIES=1` (set in .env.example) because browsers
// refuse `Secure` cookies on http://localhost.

export const INSECURE_COOKIES_ENV = "PROPHETICA_INSECURE_COOKIES";

export function secureCookies(): boolean {
  return process.env[INSECURE_COOKIES_ENV] !== "1";
}
