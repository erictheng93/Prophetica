export const SESSION_COOKIE_NAME = "proph_session";

const DEFAULT_AUTH_BASE_URL = "http://127.0.0.1:8084";
const LOGIN_PATH = "/api/auth/login";
const SESSION_HMAC_ALGORITHM = { name: "HMAC", hash: "SHA-256" } as const;

export type LoginIdentifierKind = "email" | "phone";

export interface NormalizedLoginIdentifier {
  kind: LoginIdentifierKind;
  canonical: string;
}

export interface AuthenticateWithPasswordInput {
  identifier: string;
  password: string;
  remember: boolean;
  mfaCode?: string;
  mfaRecoveryCode?: string;
}

export type PasswordAuthResult =
  | {
      status: "authenticated";
      userId: string;
      sessionToken: string;
      expiresAt?: string;
    }
  | {
      status: "mfa_required";
      userId: string;
      challengeId: string;
    }
  | { status: "invalid_identifier" }
  | { status: "invalid_credentials" }
  | { status: "unavailable" };

export interface AuthenticateWithPasswordOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
}

export interface SessionPayload {
  userId: string;
  issuedAtEpochSeconds: number;
  expiresAtEpochSeconds: number;
}

export interface SessionTokenOptions {
  secret?: string;
}

export interface VerifySessionTokenOptions extends SessionTokenOptions {
  nowEpochSeconds?: number;
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

interface AuthServicePayload {
  user_id?: unknown;
  session_token?: unknown;
  token?: unknown;
  expires_at?: unknown;
  mfa_required?: unknown;
  challenge_id?: unknown;
}

export function normalizeLoginIdentifier(
  value: string,
): NormalizedLoginIdentifier | null {
  const trimmed = value.trim();

  if (trimmed.includes("@")) {
    const email = trimmed.toLowerCase();
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
      ? { kind: "email", canonical: email }
      : null;
  }

  const compact = trimmed.replace(/[\s().-]/g, "");
  if (!/^\+?\d+$/.test(compact)) {
    return null;
  }

  const digits = compact.startsWith("+") ? compact.slice(1) : compact;
  const subscriber = digits.startsWith("886")
    ? digits.slice(3)
    : digits.startsWith("0")
      ? digits.slice(1)
      : digits;

  if (!/^9\d{8}$/.test(subscriber)) {
    return null;
  }

  return { kind: "phone", canonical: `+886${subscriber}` };
}

export async function authenticateWithPassword(
  input: AuthenticateWithPasswordInput,
  options: AuthenticateWithPasswordOptions = {},
): Promise<PasswordAuthResult> {
  const identifier = normalizeLoginIdentifier(input.identifier);

  if (!identifier) {
    return { status: "invalid_identifier" };
  }
  if (input.password.length === 0) {
    return { status: "invalid_credentials" };
  }

  const baseUrl = trimTrailingSlash(
    options.baseUrl ?? process.env.USER_SERVICE_URL ?? DEFAULT_AUTH_BASE_URL,
  );
  const fetcher = options.fetcher ?? fetch;

  try {
    const response = await fetcher(`${baseUrl}${LOGIN_PATH}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        identifier: identifier.canonical,
        identifier_kind: identifier.kind,
        password: input.password,
        remember: input.remember,
        login_surface: "user_web",
        mfa_code: nonEmptyString(input.mfaCode),
        mfa_recovery_code: nonEmptyString(input.mfaRecoveryCode),
      }),
    });

    if (response.status === 401 || response.status === 403) {
      return { status: "invalid_credentials" };
    }
    if (!response.ok) {
      return { status: "unavailable" };
    }

    return mapAuthServicePayload((await response.json()) as AuthServicePayload);
  } catch {
    return { status: "unavailable" };
  }
}

export async function createSessionToken(
  payload: SessionPayload,
  options: SessionTokenOptions = {},
): Promise<string> {
  const encodedPayload = base64UrlEncode(
    new TextEncoder().encode(JSON.stringify(payload)),
  );
  const signature = await signSessionPayload(encodedPayload, sessionSecret(options.secret));

  return `${encodedPayload}.${signature}`;
}

export async function verifySessionToken(
  token: string,
  options: VerifySessionTokenOptions = {},
): Promise<SessionPayload | null> {
  const parts = token.split(".");
  if (parts.length !== 2) {
    return null;
  }

  const [encodedPayload, signature] = parts;
  if (!encodedPayload || !signature) {
    return null;
  }

  const expectedSignature = await signSessionPayload(
    encodedPayload,
    sessionSecret(options.secret),
  );

  if (!constantTimeEqual(signature, expectedSignature)) {
    return null;
  }

  const payload = decodeSessionPayload(encodedPayload);
  if (!payload) {
    return null;
  }

  const now = options.nowEpochSeconds ?? Math.floor(Date.now() / 1000);
  return payload.expiresAtEpochSeconds >= now ? payload : null;
}

function mapAuthServicePayload(payload: AuthServicePayload): PasswordAuthResult {
  const userId = typeof payload.user_id === "string" ? payload.user_id : "";

  if (payload.mfa_required === true && userId) {
    const challengeId =
      typeof payload.challenge_id === "string" ? payload.challenge_id : "";
    return challengeId
      ? { status: "mfa_required", userId, challengeId }
      : { status: "unavailable" };
  }

  const sessionToken =
    typeof payload.session_token === "string"
      ? payload.session_token
      : typeof payload.token === "string"
        ? payload.token
        : "";
  const expiresAt =
    typeof payload.expires_at === "string" ? payload.expires_at : undefined;

  return userId && sessionToken
    ? { status: "authenticated", userId, sessionToken, expiresAt }
    : { status: "unavailable" };
}

async function signSessionPayload(payload: string, secret: string): Promise<string> {
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    SESSION_HMAC_ALGORITHM,
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    SESSION_HMAC_ALGORITHM.name,
    key,
    encoder.encode(payload),
  );

  return base64UrlEncode(new Uint8Array(signature));
}

function decodeSessionPayload(value: string): SessionPayload | null {
  try {
    const payload = JSON.parse(
      new TextDecoder().decode(base64UrlDecode(value)),
    ) as Partial<SessionPayload>;

    if (
      typeof payload.userId !== "string" ||
      typeof payload.issuedAtEpochSeconds !== "number" ||
      typeof payload.expiresAtEpochSeconds !== "number"
    ) {
      return null;
    }

    return {
      userId: payload.userId,
      issuedAtEpochSeconds: payload.issuedAtEpochSeconds,
      expiresAtEpochSeconds: payload.expiresAtEpochSeconds,
    };
  } catch {
    return null;
  }
}

function sessionSecret(override?: string): string {
  if (override) {
    return override;
  }

  const configuredSecret =
    process.env.PROPHETICA_SESSION_SECRET ?? process.env.AUTH_SESSION_SECRET;

  if (configuredSecret) {
    return configuredSecret;
  }

  if (process.env.NODE_ENV === "production") {
    throw new Error("PROPHETICA_SESSION_SECRET is required in production");
  }

  return "local-development-session-secret";
}

function base64UrlEncode(value: Uint8Array): string {
  let binary = "";
  for (const byte of value) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "");
}

function base64UrlDecode(value: string): Uint8Array {
  const padded = value.padEnd(value.length + ((4 - (value.length % 4)) % 4), "=");
  const base64 = padded.replaceAll("-", "+").replaceAll("_", "/");
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes;
}

function constantTimeEqual(left: string, right: string): boolean {
  if (left.length !== right.length) {
    return false;
  }

  let diff = 0;
  for (let index = 0; index < left.length; index += 1) {
    diff |= left.charCodeAt(index) ^ right.charCodeAt(index);
  }

  return diff === 0;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function nonEmptyString(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}
