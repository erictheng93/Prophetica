import type { SessionPayload } from "./session-crypto.js";

const STEP_UP_WINDOW_SECONDS = 30 * 60;
const SAFE_METHODS = new Set(["GET", "HEAD", "OPTIONS"]);

const STEP_UP_PATH_PREFIXES = [
  "/approvals",
  "/kyc-risk",
  "/api/members/actions",
  "/api/points/manual-adjustments",
  "/affiliate/commissions",
  "/affiliate/payouts",
];

export function requiresStepUpMfa(pathname: string, method: string): boolean {
  if (SAFE_METHODS.has(method.toUpperCase())) return false;
  return STEP_UP_PATH_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

export function hasRecentHardwareMfa(
  session: SessionPayload,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): boolean {
  if (typeof session.mfaAt !== "number") return false;
  if (!Array.isArray(session.amr) || !session.amr.includes("hwk")) return false;
  return session.mfaAt > 0 && nowSeconds - session.mfaAt <= STEP_UP_WINDOW_SECONDS;
}
