import assert from "node:assert/strict";
import { test } from "node:test";

import {
  authenticateWithPassword,
  createSessionToken,
  normalizeLoginIdentifier,
  verifySessionToken,
} from "../lib/auth.js";
import { registerPublicAccount } from "../lib/signup.js";

test("normalizeLoginIdentifier lowercases email identifiers", () => {
  assert.deepEqual(normalizeLoginIdentifier("  Member@Example.COM  "), {
    kind: "email",
    canonical: "member@example.com",
  });
});

test("normalizeLoginIdentifier canonicalizes Taiwan mobile numbers with or without leading zero", () => {
  const expected = {
    kind: "phone",
    canonical: "+886912345698",
  };

  assert.deepEqual(normalizeLoginIdentifier("0912345698"), expected);
  assert.deepEqual(normalizeLoginIdentifier("912345698"), expected);
  assert.deepEqual(normalizeLoginIdentifier("+886 912-345-698"), expected);
  assert.deepEqual(normalizeLoginIdentifier("886912345698"), expected);
});

test("normalizeLoginIdentifier rejects unsupported identifiers", () => {
  assert.equal(normalizeLoginIdentifier("not-an-email"), null);
  assert.equal(normalizeLoginIdentifier("0812345698"), null);
});

test("authenticateWithPassword posts canonical identifier to the auth service", async () => {
  const calls: Array<{ url: string; body: unknown }> = [];

  const result = await authenticateWithPassword(
    {
      identifier: "912345698",
      password: "correct horse battery staple",
      remember: true,
    },
    {
      baseUrl: "http://user-service.local",
      fetcher: async (input, init) => {
        calls.push({
          url: String(input),
          body: JSON.parse(String(init?.body)),
        });

        return Response.json({
          user_id: "user_42",
          session_token: "session-token-42",
          expires_at: "2026-05-27T00:00:00.000Z",
        });
      },
    },
  );

  assert.deepEqual(calls, [
    {
      url: "http://user-service.local/api/auth/login",
      body: {
        identifier: "+886912345698",
        identifier_kind: "phone",
        password: "correct horse battery staple",
        remember: true,
        login_surface: "user_web",
      },
    },
  ]);
  assert.deepEqual(result, {
    status: "authenticated",
    userId: "user_42",
    sessionToken: "session-token-42",
    expiresAt: "2026-05-27T00:00:00.000Z",
  });
});

test("authenticateWithPassword forwards optional MFA step-up evidence", async () => {
  const calls: Array<{ body: unknown }> = [];

  const result = await authenticateWithPassword(
    {
      identifier: "member@example.com",
      password: "correct horse battery staple",
      remember: false,
      mfaCode: " 123456 ",
      mfaRecoveryCode: "",
    },
    {
      baseUrl: "http://user-service.local",
      fetcher: async (_input, init) => {
        calls.push({
          body: JSON.parse(String(init?.body)),
        });

        return Response.json({
          user_id: "user_42",
          session_token: "session-token-42",
        });
      },
    },
  );

  assert.deepEqual(calls, [
    {
      body: {
        identifier: "member@example.com",
        identifier_kind: "email",
        password: "correct horse battery staple",
        remember: false,
        login_surface: "user_web",
        mfa_code: "123456",
      },
    },
  ]);
  assert.equal(result.status, "authenticated");
});

test("authenticateWithPassword maps invalid and unavailable responses", async () => {
  const invalid = await authenticateWithPassword(
    { identifier: "member@example.com", password: "wrong", remember: false },
    {
      baseUrl: "http://user-service.local",
      fetcher: async () => new Response("{}", { status: 401 }),
    },
  );
  const unavailable = await authenticateWithPassword(
    { identifier: "member@example.com", password: "secret", remember: false },
    {
      baseUrl: "http://user-service.local",
      fetcher: async () => {
        throw new Error("connection refused");
      },
    },
  );

  assert.deepEqual(invalid, { status: "invalid_credentials" });
  assert.deepEqual(unavailable, { status: "unavailable" });
});

test("registerPublicAccount submits normalized signup data with referral attribution", async () => {
  const calls: Array<{ url: string; body: unknown }> = [];

  const result = await registerPublicAccount(
    {
      email: " New.Member@Example.COM ",
      phone: "0912-345-698",
      password: "correct horse battery staple",
      displayName: " New Member ",
      referralCode: " AFF_2026 ",
    },
    {
      baseUrl: "http://user-service.local/",
      referralCaptureBaseUrl: "http://api-gateway.local",
      fetcher: async (input, init) => {
        calls.push({
          url: String(input),
          body: JSON.parse(String(init?.body)),
        });

        if (String(input).endsWith("/api/internal/referrals/capture")) {
          return Response.json({
            captured: true,
          });
        }

        return Response.json({
          user_id: "MEM-2026",
        });
      },
    },
  );

  assert.deepEqual(calls, [
    {
      url: "http://user-service.local/v1/public/registrations",
      body: {
        email: "new.member@example.com",
        phone: "+886912345698",
        password: "correct horse battery staple",
        display_name: "New Member",
        referral_code: "AFF_2026",
      },
    },
    {
      url: "http://api-gateway.local/api/internal/referrals/capture",
      body: {
        referral_code: "AFF_2026",
        referred_user_id: "MEM-2026",
        source: "public-web-signup",
      },
    },
  ]);
  assert.deepEqual(result, { status: "registered", userId: "MEM-2026" });
});

test("registerPublicAccount fails closed when referral capture is unavailable", async () => {
  const result = await registerPublicAccount(
    {
      email: "member@example.com",
      phone: "0912345698",
      password: "correct horse battery staple",
      displayName: "Member",
      referralCode: "AFF_2026",
    },
    {
      fetcher: async (input) => {
        if (String(input).endsWith("/api/internal/referrals/capture")) {
          return new Response("{}", { status: 502 });
        }

        return Response.json({
          user_id: "MEM-2026",
        });
      },
    },
  );

  assert.deepEqual(result, { status: "unavailable" });
});

test("registerPublicAccount rejects invalid local input before calling registration service", async () => {
  let called = false;

  const result = await registerPublicAccount(
    {
      email: "not-an-email",
      phone: "0812345698",
      password: "short",
      displayName: "",
      referralCode: "AFF_2026",
    },
    {
      fetcher: async () => {
        called = true;
        return Response.json({});
      },
    },
  );

  assert.deepEqual(result, { status: "invalid" });
  assert.equal(called, false);
});

test("session tokens are signed and expire", async () => {
  const token = await createSessionToken(
    {
      userId: "user_42",
      issuedAtEpochSeconds: 1_780_000_000,
      expiresAtEpochSeconds: 1_780_000_600,
    },
    { secret: "test-session-secret" },
  );

  assert.deepEqual(
    await verifySessionToken(token, {
      secret: "test-session-secret",
      nowEpochSeconds: 1_780_000_100,
    }),
    {
      userId: "user_42",
      issuedAtEpochSeconds: 1_780_000_000,
      expiresAtEpochSeconds: 1_780_000_600,
    },
  );
  assert.equal(
    await verifySessionToken(token, {
      secret: "test-session-secret",
      nowEpochSeconds: 1_780_000_601,
    }),
    null,
  );
  assert.equal(
    await verifySessionToken(`${token}.tampered`, {
      secret: "test-session-secret",
      nowEpochSeconds: 1_780_000_100,
    }),
    null,
  );
});

test("production session tokens require an explicit secret", async () => {
  const mutableEnv = process.env as Record<string, string | undefined>;
  const previousNodeEnv = process.env.NODE_ENV;
  const previousSessionSecret = process.env.PROPHETICA_SESSION_SECRET;
  const previousAuthSessionSecret = process.env.AUTH_SESSION_SECRET;

  mutableEnv.NODE_ENV = "production";
  delete mutableEnv.PROPHETICA_SESSION_SECRET;
  delete mutableEnv.AUTH_SESSION_SECRET;

  try {
    await assert.rejects(
      createSessionToken({
        userId: "user_42",
        issuedAtEpochSeconds: 1_780_000_000,
        expiresAtEpochSeconds: 1_780_000_600,
      }),
      /PROPHETICA_SESSION_SECRET/,
    );
  } finally {
    if (previousNodeEnv === undefined) {
      delete mutableEnv.NODE_ENV;
    } else {
      mutableEnv.NODE_ENV = previousNodeEnv;
    }
    if (previousSessionSecret === undefined) {
      delete mutableEnv.PROPHETICA_SESSION_SECRET;
    } else {
      mutableEnv.PROPHETICA_SESSION_SECRET = previousSessionSecret;
    }
    if (previousAuthSessionSecret === undefined) {
      delete mutableEnv.AUTH_SESSION_SECRET;
    } else {
      mutableEnv.AUTH_SESSION_SECRET = previousAuthSessionSecret;
    }
  }
});
