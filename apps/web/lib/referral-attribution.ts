export const REFERRAL_ATTRIBUTION_COOKIE = "proph_referral_code";
export const REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS = 60 * 60 * 24 * 30;

const REFERRAL_QUERY_KEYS = ["referral_code", "ref", "affiliate_code"];
const REFERRAL_CODE_PATTERN = /^[A-Za-z0-9_-]{3,64}$/;

export function referralCodeFromSearchParams(searchParams: URLSearchParams): string | null {
  for (const key of REFERRAL_QUERY_KEYS) {
    const value = normalizeReferralCode(searchParams.get(key));
    if (value) return value;
  }
  return null;
}

export function normalizeReferralCode(value: string | null): string | null {
  const trimmed = value?.trim();
  if (!trimmed || !REFERRAL_CODE_PATTERN.test(trimmed)) return null;
  return trimmed;
}

