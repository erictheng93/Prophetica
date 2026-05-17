const DEFAULT_REGISTRATION_BASE_URL = "http://127.0.0.1:8084";
const DEFAULT_REFERRAL_CAPTURE_BASE_URL = "http://127.0.0.1:8080";
const REGISTRATION_PATH = "/v1/public/registrations";
const REFERRAL_CAPTURE_PATH = "/api/internal/referrals/capture";
const REFERRAL_CODE_PATTERN = /^[A-Za-z0-9_-]{3,64}$/;

export type SignupResult =
  | { status: "registered"; userId: string }
  | { status: "invalid" }
  | { status: "conflict" }
  | { status: "unavailable" };

export interface RegisterPublicAccountInput {
  email: string;
  phone: string;
  password: string;
  displayName: string;
  referralCode?: string | null;
}

export interface RegisterPublicAccountOptions {
  baseUrl?: string;
  referralCaptureBaseUrl?: string;
  fetcher?: Fetcher;
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

interface RegistrationServicePayload {
  user_id?: unknown;
  member?: {
    user_id?: unknown;
  };
}

export async function registerPublicAccount(
  input: RegisterPublicAccountInput,
  options: RegisterPublicAccountOptions = {},
): Promise<SignupResult> {
  const email = normalizeSignupEmail(input.email);
  const phone = normalizeSignupPhone(input.phone);
  const password = input.password.trim();
  const displayName = input.displayName.trim();

  if (!email || !phone || password.length < 8 || displayName.length < 1) {
    return { status: "invalid" };
  }

  const baseUrl = trimTrailingSlash(
    options.baseUrl ??
      process.env.USER_SERVICE_URL ??
      DEFAULT_REGISTRATION_BASE_URL,
  );
  const fetcher = options.fetcher ?? fetch;
  const referralCode = normalizeReferralCode(input.referralCode ?? null);

  try {
    const response = await fetcher(`${baseUrl}${REGISTRATION_PATH}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        email,
        phone,
        password,
        display_name: displayName,
        ...(referralCode ? { referral_code: referralCode } : {}),
      }),
    });

    if (response.status === 400 || response.status === 422) {
      return { status: "invalid" };
    }
    if (response.status === 409) {
      return { status: "conflict" };
    }
    if (!response.ok) {
      return { status: "unavailable" };
    }

    const registered = mapRegistrationServicePayload(
      (await response.json()) as RegistrationServicePayload,
    );
    if (registered.status !== "registered" || !referralCode) {
      return registered;
    }

    return (await captureSignupReferral(
      registered.userId,
      referralCode,
      options,
    ))
      ? registered
      : { status: "unavailable" };
  } catch {
    return { status: "unavailable" };
  }
}

function mapRegistrationServicePayload(
  payload: RegistrationServicePayload,
): SignupResult {
  const userId =
    typeof payload.user_id === "string"
      ? payload.user_id
      : typeof payload.member?.user_id === "string"
        ? payload.member.user_id
        : "";

  return userId ? { status: "registered", userId } : { status: "unavailable" };
}

function normalizeSignupEmail(value: string): string | null {
  const email = value.trim().toLowerCase();
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) ? email : null;
}

function normalizeSignupPhone(value: string): string | null {
  const compact = value.trim().replace(/[\s().-]/g, "");
  if (!/^\+?\d+$/.test(compact)) {
    return null;
  }
  const digits = compact.startsWith("+") ? compact.slice(1) : compact;
  const subscriber = digits.startsWith("886")
    ? digits.slice(3)
    : digits.startsWith("0")
      ? digits.slice(1)
      : digits;

  return /^9\d{8}$/.test(subscriber) ? `+886${subscriber}` : null;
}

async function captureSignupReferral(
  userId: string,
  referralCode: string,
  options: RegisterPublicAccountOptions,
): Promise<boolean> {
  const baseUrl = trimTrailingSlash(
    options.referralCaptureBaseUrl ??
      process.env.API_GATEWAY_URL ??
      DEFAULT_REFERRAL_CAPTURE_BASE_URL,
  );
  const fetcher = options.fetcher ?? fetch;

  try {
    const response = await fetcher(`${baseUrl}${REFERRAL_CAPTURE_PATH}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        referral_code: referralCode,
        referred_user_id: userId,
        source: "public-web-signup",
      }),
    });

    return response.ok;
  } catch {
    return false;
  }
}

function normalizeReferralCode(value: string | null): string | null {
  const trimmed = value?.trim();
  if (!trimmed || !REFERRAL_CODE_PATTERN.test(trimmed)) {
    return null;
  }
  return trimmed;
}

function trimTrailingSlash(value: string): string {
  return value.replace(/\/+$/, "");
}
