import assert from "node:assert/strict";
import { test } from "node:test";

import {
  LANDING_ANALYTICS_EVENTS,
  createAnalyticsClient,
} from "../src/analytics/index.js";
import { createAnalyticsClient as createRootAnalyticsClient } from "../src/index.js";

const validDimensions = {
  variant_id: "hero-a",
  product_line: "sports",
  region: "US",
  cta_id: "join-now",
  placement: "hero",
  destination: "/signup",
};

test("landing analytics dictionary defines the shared CTA event contract", () => {
  assert.deepEqual(LANDING_ANALYTICS_EVENTS.landing_cta_clicked, {
    event_name: "landing_cta_clicked",
    schema_version: "analytics.v1",
    dimensions: [
      "variant_id",
      "product_line",
      "region",
      "cta_id",
      "placement",
      "destination",
    ],
  });
});

test("analytics client emits landing CTA payloads with sendBeacon when available", async () => {
  const beacons: Array<{ endpoint: string; payload: unknown }> = [];
  const client = createAnalyticsClient({
    endpoint: "/collect",
    now: () => new Date("2026-05-16T00:00:00.000Z"),
    navigator: {
      sendBeacon: (endpoint, body) => {
        beacons.push({
          endpoint,
          payload: JSON.parse(String(body)),
        });
        return true;
      },
    },
    fetcher: async () => {
      throw new Error("fetch should not be called when sendBeacon succeeds");
    },
  });

  const emitted = await client.trackLandingCtaClicked(validDimensions);

  assert.equal(emitted, true);
  assert.deepEqual(beacons, [
    {
      endpoint: "/collect",
      payload: {
        event_name: "landing_cta_clicked",
        schema_version: "analytics.v1",
        occurred_at: "2026-05-16T00:00:00.000Z",
        dimensions: validDimensions,
      },
    },
  ]);
});

test("analytics client falls back to fetch with keepalive JSON POST", async () => {
  const calls: Array<{
    endpoint: string;
    init: RequestInit | undefined;
    payload: unknown;
  }> = [];
  const client = createAnalyticsClient({
    navigator: {},
    fetcher: async (endpoint, init) => {
      calls.push({
        endpoint: String(endpoint),
        init,
        payload: JSON.parse(String(init?.body)),
      });
      return new Response(null, { status: 204 });
    },
    now: () => new Date("2026-05-16T00:00:00.000Z"),
  });

  const emitted = await client.trackLandingCtaClicked(validDimensions);

  assert.equal(emitted, true);
  assert.equal(calls[0]?.endpoint, "/api/analytics/events");
  assert.equal(calls[0]?.init?.method, "POST");
  assert.equal(calls[0]?.init?.keepalive, true);
  assert.deepEqual(Object.fromEntries(new Headers(calls[0]?.init?.headers)), {
    "content-type": "application/json",
  });
  assert.deepEqual(calls[0]?.payload, {
    event_name: "landing_cta_clicked",
    schema_version: "analytics.v1",
    occurred_at: "2026-05-16T00:00:00.000Z",
    dimensions: validDimensions,
  });
});

test("analytics client returns false without transport or when disabled", async () => {
  const withoutTransport = createAnalyticsClient({
    navigator: {},
    fetcher: undefined,
  });
  const disabled = createAnalyticsClient({
    enabled: false,
    navigator: {
      sendBeacon: () => {
        throw new Error("disabled client should not send");
      },
    },
  });

  assert.equal(
    await withoutTransport.trackLandingCtaClicked(validDimensions),
    false,
  );
  assert.equal(await disabled.trackLandingCtaClicked(validDimensions), false);
});

test("analytics client rejects missing required landing CTA dimensions deterministically", async () => {
  const client = createAnalyticsClient({
    navigator: {
      sendBeacon: () => {
        throw new Error("invalid payload should not send");
      },
    },
  });

  const emitted = await client.trackLandingCtaClicked({
    ...validDimensions,
    variant_id: "",
  });

  assert.equal(emitted, false);
});

test("root ts-sdk index exports analytics client factory", () => {
  assert.equal(createRootAnalyticsClient, createAnalyticsClient);
});
