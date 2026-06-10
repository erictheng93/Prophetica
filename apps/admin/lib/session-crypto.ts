import { isRole, type Role } from "./rbac.js";

/*
 * Pure HMAC envelope split out of lib/session.ts so node:test can exercise
 * sign/verify without pulling in `server-only` or `next/headers`. The
 * cookie-binding side lives in lib/session.ts and is only used at runtime.
 */

const SESSION_TTL_SECONDS = 60 * 60 * 8;
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";

export interface SessionPayload {
  sub: string;
  role: Role;
  iat: number;
  exp: number;
  affiliateId?: string;
}

interface JwsHeader {
  alg?: string;
  kid?: string;
  typ?: string;
}

type SessionJwk = JsonWebKey & {
  kid?: string;
  kty?: string;
  crv?: string;
};

export function resolveSessionSecret(
  env: Record<string, string | undefined> = process.env,
): string {
  const configured = env.ADMIN_SESSION_SECRET;
  if (configured?.trim()) return configured;
  if (env.NODE_ENV === "production") {
    throw new Error("ADMIN_SESSION_SECRET is required in production");
  }
  return DEV_SECRET;
}

export function resolveSessionJwks(
  env: Record<string, string | undefined> = process.env,
): SessionJwk[] {
  const jwks = env.ADMIN_SESSION_JWKS?.trim();
  if (jwks) {
    const parsed = JSON.parse(jwks) as { keys?: SessionJwk[] };
    return Array.isArray(parsed.keys) ? parsed.keys : [];
  }
  const publicJwk = env.ADMIN_SESSION_ED25519_PUBLIC_JWK?.trim();
  if (publicJwk) return [JSON.parse(publicJwk) as SessionJwk];
  return [];
}

function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function base64UrlDecode(input: string): Uint8Array {
  const padded = input
    .replace(/-/g, "+")
    .replace(/_/g, "/")
    .padEnd(input.length + ((4 - (input.length % 4)) % 4), "=");
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function importKey(secret: string): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign", "verify"],
  );
}

function constantTimeEquals(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

function parseSessionPayload(
  payload: unknown,
  nowSeconds: number,
): SessionPayload | null {
  if (!payload || typeof payload !== "object") return null;
  const candidate = payload as Record<string, unknown>;
  if (
    typeof candidate.sub !== "string" ||
    typeof candidate.iat !== "number" ||
    typeof candidate.exp !== "number" ||
    !isRole(candidate.role)
  ) {
    return null;
  }
  if (candidate.exp < nowSeconds) return null;

  const result: SessionPayload = {
    sub: candidate.sub,
    role: candidate.role,
    iat: candidate.iat,
    exp: candidate.exp,
  };
  if (typeof candidate.affiliateId === "string") {
    result.affiliateId = candidate.affiliateId;
  }
  return result;
}

export async function signSession(
  payload: Omit<SessionPayload, "iat" | "exp">,
  secret: string,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): Promise<string> {
  const full: SessionPayload = {
    ...payload,
    iat: nowSeconds,
    exp: nowSeconds + SESSION_TTL_SECONDS,
  };
  const body = base64UrlEncode(new TextEncoder().encode(JSON.stringify(full)));
  const key = await importKey(secret);
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(body),
  );
  return `${body}.${base64UrlEncode(new Uint8Array(sig))}`;
}

export async function verifySession(
  token: string,
  secret: string,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): Promise<SessionPayload | null> {
  const parts = token.split(".");
  if (parts.length !== 2) return null;
  const [body, sig] = parts;

  const key = await importKey(secret);
  const expected = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(body),
  );
  const expectedB64 = base64UrlEncode(new Uint8Array(expected));
  if (!constantTimeEquals(expectedB64, sig)) return null;

  let payload: unknown;
  try {
    payload = JSON.parse(new TextDecoder().decode(base64UrlDecode(body)));
  } catch {
    return null;
  }
  return parseSessionPayload(payload, nowSeconds);
}

export async function verifyEd25519JwsSession(
  token: string,
  publicKeys: SessionJwk[],
  nowSeconds: number = Math.floor(Date.now() / 1000),
): Promise<SessionPayload | null> {
  const parts = token.split(".");
  if (parts.length !== 3 || publicKeys.length === 0) return null;
  const [headerB64, payloadB64, sigB64] = parts;

  let header: JwsHeader;
  let payload: unknown;
  try {
    header = JSON.parse(
      new TextDecoder().decode(base64UrlDecode(headerB64)),
    ) as JwsHeader;
    payload = JSON.parse(new TextDecoder().decode(base64UrlDecode(payloadB64)));
  } catch {
    return null;
  }
  if (header.alg !== "EdDSA") return null;

  const candidateKeys = publicKeys.filter((key) => {
    if (key.kty !== "OKP" || key.crv !== "Ed25519") return false;
    return !header.kid || !key.kid || key.kid === header.kid;
  });
  if (candidateKeys.length === 0) return null;

  const data = new TextEncoder().encode(`${headerB64}.${payloadB64}`);
  const signature = base64UrlDecode(sigB64);
  for (const jwk of candidateKeys) {
    try {
      const key = await crypto.subtle.importKey(
        "jwk",
        jwk,
        { name: "Ed25519" } as AlgorithmIdentifier,
        false,
        ["verify"],
      );
      const valid = await crypto.subtle.verify(
        { name: "Ed25519" } as AlgorithmIdentifier,
        key,
        signature as unknown as BufferSource,
        data as unknown as BufferSource,
      );
      if (valid) return parseSessionPayload(payload, nowSeconds);
    } catch {
      continue;
    }
  }
  return null;
}

export async function verifyAdminSessionToken(
  token: string,
  env: Record<string, string | undefined> = process.env,
  nowSeconds: number = Math.floor(Date.now() / 1000),
): Promise<SessionPayload | null> {
  if (token.split(".").length === 3) {
    return verifyEd25519JwsSession(token, resolveSessionJwks(env), nowSeconds);
  }

  if (env.NODE_ENV === "production") {
    return null;
  }

  return verifySession(token, resolveSessionSecret(env), nowSeconds);
}
