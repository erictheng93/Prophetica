import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchSelfExclusionStatus,
  requestSelfExclusion,
} from "../lib/responsible-gaming-api.js";

test("fetchSelfExclusionStatus returns fallback without bearer auth", async () => {
  let called = false;
  const status = await fetchSelfExclusionStatus({
    baseUrl: "http://gateway.local",
    userId: "user_42",
    fetcher: async (): Promise<Response> => {
      called = true;
      return Response.json({});
    },
  });

  assert.equal(called, false);
  assert.equal(status.source, "fallback:api-gateway-unavailable");
  assert.equal(status.userId, "user_42");
});

test("fetchSelfExclusionStatus maps live api-gateway response with bearer auth", async () => {
  const calls: Array<{
    url: string;
    authorization: string | null;
    userIdHeader: string | null;
  }> = [];

  const status = await fetchSelfExclusionStatus({
    baseUrl: "http://gateway.local",
    userId: "user_42",
    bearerToken: "jwt-user-service",
    fetcher: async (
      input: string | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      const headers = new Headers(init?.headers);
      calls.push({
        url: String(input),
        authorization: headers.get("authorization"),
        userIdHeader: headers.get("x-user-id"),
      });
      return Response.json({
        user_id: "user_42",
        active: true,
        record: {
          record_id: "se_user_42_1780000000",
          user_id: "user_42",
          mode: "timeout",
          period: "one_year",
          points_disposition: null,
          reason: null,
          starts_at_epoch_seconds: 1_780_000_000,
          ends_at_epoch_seconds: 1_811_536_000,
          frozen_minor_units: 12_500,
        },
        source: "api-gateway",
      });
    },
  });

  assert.equal(
    calls[0]?.url,
    "http://gateway.local/v1/responsible-gaming/self-exclusion",
  );
  assert.equal(calls[0]?.authorization, "Bearer jwt-user-service");
  assert.equal(calls[0]?.userIdHeader, null);
  assert.equal(status.source, "api-gateway");
  assert.equal(status.active, true);
  assert.equal(status.record?.frozenPoints, 125);
});

test("fetchSelfExclusionStatus does not call api-gateway without bearer token by default", async () => {
  let called = false;
  const status = await fetchSelfExclusionStatus({
    baseUrl: "http://gateway.local",
    userId: "user_42",
    fetcher: async (): Promise<Response> => {
      called = true;
      return Response.json({});
    },
  });

  assert.equal(called, false);
  assert.equal(status.source, "fallback:api-gateway-unavailable");
  assert.equal(status.userId, "user_42");
});

test("fetchSelfExclusionStatus forwards user-service bearer token when available", async () => {
  const calls: Array<{ headers: Record<string, string> }> = [];
  await fetchSelfExclusionStatus({
    baseUrl: "http://gateway.local",
    userId: "user_42",
    bearerToken: "jwt-user-service",
    fetcher: async (_input, init): Promise<Response> => {
      calls.push({
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
      });
      return Response.json({
        user_id: "user_42",
        active: false,
        record: null,
      });
    },
  });

  assert.equal(calls[0]?.headers.authorization, "Bearer jwt-user-service");
  assert.equal(calls[0]?.headers["x-user-id"], undefined);
});

test("requestSelfExclusion posts through the SDK client and returns fallback when unavailable", async () => {
  const calls: unknown[] = [];
  const response = await requestSelfExclusion({
    baseUrl: "http://gateway.local",
    userId: "user_42",
    mode: "timeout",
    period: "six_months",
    consent: true,
    bearerToken: "jwt-user-service",
    responsibleGamingClientFactory: ({ auth }) => ({
      async getSelfExclusionStatus() {
        throw new Error("not used");
      },
      async submitSelfExclusion(request) {
        calls.push({ auth, request });
        return {
          record: {
            recordId: "se_user_42_1780000000",
            userId: "user_42",
            mode: "timeout",
            period: "six_months",
            pointsDisposition: null,
            reason: null,
            startsAtEpochSeconds: 1_780_000_000,
            endsAtEpochSeconds: 1_795_811_200,
            frozenMinorUnits: 12_500n,
          },
          ledgerIdempotencyKey: "self-exclusion:user_42:test",
          replayed: false,
        };
      },
    }),
  });

  assert.equal(response.source, "api-gateway");
  if (response.source === "api-gateway") {
    assert.equal(response.record.frozenPoints, 125);
  }
  assert.deepEqual(calls, [
    {
      auth: { bearerToken: "jwt-user-service" },
      request: {
        mode: "timeout",
        period: "six_months",
        consent: true,
      },
    },
  ]);

  const fallback = await requestSelfExclusion({
    mode: "timeout",
    period: "six_months",
    consent: true,
    responsibleGamingClientFactory: () => {
      throw new Error("factory should not be called without bearer");
    },
  });
  assert.equal(fallback.source, "fallback:api-gateway-unavailable");
});

test("requestSelfExclusion skips client calls without bearer auth", async () => {
  const fallback = await requestSelfExclusion({
    mode: "timeout",
    period: "six_months",
    consent: true,
    responsibleGamingClientFactory: () => ({
      async getSelfExclusionStatus() {
        throw new Error("not used");
      },
      async submitSelfExclusion() {
        throw new Error("connection refused");
      },
    }),
  });
  assert.equal(fallback.source, "fallback:api-gateway-unavailable");
});
