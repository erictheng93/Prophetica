import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchCooldownStatus,
  startCooldown,
  type CooldownDuration,
} from "../lib/responsible-gaming-api.js";

test("fetchCooldownStatus calls api-gateway cooldown endpoint with auth context", async () => {
  const calls: Array<{ url: string; headers: Record<string, string> }> = [];
  const fetcher = async (
    input: string | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    calls.push({
      url: String(input),
      headers: Object.fromEntries(new Headers(init?.headers).entries()),
    });

    return Response.json({
      user_id: "user_42",
      active: true,
      duration: "24h",
      started_at_epoch_seconds: 1_800_000_000,
      ends_at_epoch_seconds: 1_800_086_400,
      remaining_seconds: 86_400,
      available_durations: ["24h", "7d", "30d"],
    });
  };

  const status = await fetchCooldownStatus({
    baseUrl: "http://api.local",
    userId: "user_42",
    fetcher,
  });

  assert.equal(
    calls[0]?.url,
    "http://api.local/v1/me/responsible-gaming/cooldown",
  );
  assert.equal(calls[0]?.headers["x-user-id"], "user_42");
  assert.equal(status.userId, "user_42");
  assert.equal(status.active, true);
  assert.equal(status.duration, "24h");
  assert.equal(status.remainingSeconds, 86_400);
  assert.deepEqual(status.availableDurations, ["24h", "7d", "30d"]);
});

test("startCooldown posts selected duration and consent to api-gateway", async () => {
  const calls: Array<{
    url: string;
    method: string | undefined;
    body: unknown;
  }> = [];
  const fetcher = async (
    input: string | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    calls.push({
      url: String(input),
      method: init?.method,
      body: JSON.parse(String(init?.body)),
    });

    return Response.json({
      user_id: "user_42",
      active: true,
      duration: "7d",
      started_at_epoch_seconds: 1_800_000_000,
      ends_at_epoch_seconds: 1_800_604_800,
      remaining_seconds: 604_800,
      available_durations: ["24h", "7d", "30d"],
    });
  };

  const status = await startCooldown(
    { duration: "7d" satisfies CooldownDuration, consent: true },
    { baseUrl: "http://api.local", userId: "user_42", fetcher },
  );

  assert.equal(
    calls[0]?.url,
    "http://api.local/v1/me/responsible-gaming/cooldown",
  );
  assert.equal(calls[0]?.method, "POST");
  assert.deepEqual(calls[0]?.body, { duration: "7d", consent: true });
  assert.equal(status.duration, "7d");
  assert.equal(status.active, true);
});

test("fetchCooldownStatus returns inactive fallback when api-gateway is unavailable", async () => {
  const status = await fetchCooldownStatus({
    baseUrl: "http://api.local",
    userId: "user_42",
    fetcher: async () => {
      throw new Error("connection refused");
    },
  });

  assert.equal(status.userId, "user_42");
  assert.equal(status.active, false);
  assert.equal(status.remainingSeconds, 0);
  assert.equal(status.source, "fallback:api-gateway-unavailable");
});
