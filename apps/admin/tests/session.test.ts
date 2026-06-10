import assert from "node:assert/strict";
import { test } from "node:test";
import {
  resolveSessionJwks,
  resolveSessionSecret,
  signSession,
  verifyAdminSessionToken,
  verifyEd25519JwsSession,
  verifySession,
} from "../lib/session-crypto.js";

const SECRET = "test-secret-do-not-use-in-prod";

test("signed session round-trips and preserves payload", async () => {
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET);
  const verified = await verifySession(token, SECRET);
  assert.ok(verified);
  assert.equal(verified.sub, "U-1");
  assert.equal(verified.role, "admin");
  assert.equal(typeof verified.iat, "number");
  assert.equal(typeof verified.exp, "number");
});

test("affiliate session preserves affiliateId", async () => {
  const token = await signSession(
    { sub: "U-2", role: "affiliate", affiliateId: "AFF-9" },
    SECRET,
  );
  const verified = await verifySession(token, SECRET);
  assert.equal(verified?.affiliateId, "AFF-9");
});

test("verifySession rejects token signed with a different secret", async () => {
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET);
  const verified = await verifySession(token, "another-secret");
  assert.equal(verified, null);
});

test("verifySession rejects tampered payload", async () => {
  const token = await signSession({ sub: "U-1", role: "manager" }, SECRET);
  const [body, sig] = token.split(".");
  // flip the role in the payload by re-encoding without re-signing
  const bodyDecoded = JSON.parse(
    Buffer.from(body, "base64url").toString("utf8"),
  );
  bodyDecoded.role = "admin";
  const tampered = `${Buffer.from(JSON.stringify(bodyDecoded)).toString("base64url")}.${sig}`;
  const verified = await verifySession(tampered, SECRET);
  assert.equal(verified, null);
});

test("verifySession rejects expired tokens", async () => {
  const longAgo = Math.floor(Date.now() / 1000) - 60 * 60 * 24 * 30;
  const token = await signSession(
    { sub: "U-1", role: "admin" },
    SECRET,
    longAgo,
  );
  const verified = await verifySession(token, SECRET);
  assert.equal(verified, null);
});

test("verifySession rejects malformed tokens", async () => {
  assert.equal(await verifySession("not-a-token", SECRET), null);
  assert.equal(await verifySession("only.one.dot.too.many", SECRET), null);
});

test("verifySession rejects unknown roles", async () => {
  const now = Math.floor(Date.now() / 1000);
  const payload = { sub: "U-1", role: "superuser", iat: now, exp: now + 60 };
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  // forge a body but no valid sig — verify must reject regardless
  const verified = await verifySession(`${body}.invalidsig`, SECRET);
  assert.equal(verified, null);
});

test("verifyAdminSessionToken rejects legacy HMAC sessions in production", async () => {
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET);

  assert.equal(
    await verifyAdminSessionToken(token, {
      NODE_ENV: "production",
      ADMIN_SESSION_SECRET: SECRET,
    }),
    null,
  );
  assert.equal(
    (await verifyAdminSessionToken(token, {
      NODE_ENV: "development",
      ADMIN_SESSION_SECRET: SECRET,
    }))?.role,
    "admin",
  );
});

test("resolveSessionSecret fails closed in production without configured secret", () => {
  assert.throws(
    () => resolveSessionSecret({ NODE_ENV: "production" }),
    /ADMIN_SESSION_SECRET is required in production/,
  );
  assert.equal(
    resolveSessionSecret({
      NODE_ENV: "production",
      ADMIN_SESSION_SECRET: "prod-secret",
    }),
    "prod-secret",
  );
});

test("verifyEd25519JwsSession accepts EdDSA compact JWS from configured JWKS", async () => {
  const pair = (await crypto.subtle.generateKey(
    { name: "Ed25519" } as AlgorithmIdentifier,
    true,
    ["sign", "verify"],
  )) as CryptoKeyPair;
  const publicJwk = await crypto.subtle.exportKey("jwk", pair.publicKey);
  jwkWithKid(publicJwk).kid = "session-key-1";
  const privateJwk = await crypto.subtle.exportKey("jwk", pair.privateKey);
  jwkWithKid(privateJwk).kid = "session-key-1";
  const now = Math.floor(Date.now() / 1000);
  const token = await signEd25519Jws(
    {
      sub: "U-3",
      role: "affiliate",
      affiliateId: "AFF-3",
      iat: now,
      exp: now + 60,
    },
    privateJwk,
  );
  const verified = await verifyEd25519JwsSession(token, [publicJwk]);

  assert.equal(verified?.sub, "U-3");
  assert.equal(verified?.role, "affiliate");
  assert.equal(verified?.affiliateId, "AFF-3");
});

test("verifyEd25519JwsSession rejects unknown kid and resolveSessionJwks parses JWKS", async () => {
  const pair = (await crypto.subtle.generateKey(
    { name: "Ed25519" } as AlgorithmIdentifier,
    true,
    ["sign", "verify"],
  )) as CryptoKeyPair;
  const publicJwk = await crypto.subtle.exportKey("jwk", pair.publicKey);
  jwkWithKid(publicJwk).kid = "session-key-1";
  const privateJwk = await crypto.subtle.exportKey("jwk", pair.privateKey);
  jwkWithKid(privateJwk).kid = "other-key";
  const now = Math.floor(Date.now() / 1000);
  const token = await signEd25519Jws(
    {
      sub: "U-3",
      role: "admin",
      iat: now,
      exp: now + 60,
    },
    privateJwk,
  );

  assert.equal(await verifyEd25519JwsSession(token, [publicJwk]), null);
  assert.equal(
    resolveSessionJwks({
      ADMIN_SESSION_JWKS: JSON.stringify({ keys: [publicJwk] }),
    })[0]?.kid,
    "session-key-1",
  );
});

test("verifyAdminSessionToken accepts Ed25519 user-service sessions from configured JWKS in production", async () => {
  const pair = (await crypto.subtle.generateKey(
    { name: "Ed25519" } as AlgorithmIdentifier,
    true,
    ["sign", "verify"],
  )) as CryptoKeyPair;
  const publicJwk = await crypto.subtle.exportKey("jwk", pair.publicKey);
  jwkWithKid(publicJwk).kid = "session-key-1";
  const privateJwk = await crypto.subtle.exportKey("jwk", pair.privateKey);
  jwkWithKid(privateJwk).kid = "session-key-1";
  const now = Math.floor(Date.now() / 1000);
  const token = await signEd25519Jws(
    {
      sub: "U-4",
      role: "manager",
      iat: now,
      exp: now + 60,
    },
    privateJwk,
  );

  const verified = await verifyAdminSessionToken(token, {
    NODE_ENV: "production",
    ADMIN_SESSION_JWKS: JSON.stringify({ keys: [publicJwk] }),
  });

  assert.equal(verified?.sub, "U-4");
  assert.equal(verified?.role, "manager");
});

async function signEd25519Jws(
  payload: unknown,
  privateJwk: JsonWebKey,
): Promise<string> {
  const header = {
    alg: "EdDSA",
    kid: jwkWithKid(privateJwk).kid,
    typ: "JWT",
  };
  const headerB64 = Buffer.from(JSON.stringify(header)).toString("base64url");
  const payloadB64 = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const data = new TextEncoder().encode(`${headerB64}.${payloadB64}`);
  const key = await crypto.subtle.importKey(
    "jwk",
    privateJwk,
    { name: "Ed25519" } as AlgorithmIdentifier,
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    { name: "Ed25519" } as AlgorithmIdentifier,
    key,
    data as unknown as BufferSource,
  );
  return `${headerB64}.${payloadB64}.${Buffer.from(signature).toString("base64url")}`;
}

function jwkWithKid(jwk: JsonWebKey): JsonWebKey & { kid?: string } {
  return jwk as JsonWebKey & { kid?: string };
}
