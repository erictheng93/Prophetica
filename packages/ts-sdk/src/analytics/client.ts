import {
  LANDING_ANALYTICS_EVENTS,
  type LandingCtaClickedDimensions,
  type LandingCtaClickedPayload,
} from "./events.js";

const DEFAULT_ANALYTICS_ENDPOINT = "/api/analytics/events";

type BeaconLike = (url: string, data?: BodyInit | null) => boolean;

export type AnalyticsFetcher = (
  input: string,
  init?: RequestInit,
) => Promise<Response>;

export type AnalyticsNavigator = {
  sendBeacon?: BeaconLike;
};

export type AnalyticsClientOptions = {
  endpoint?: string;
  enabled?: boolean;
  navigator?: AnalyticsNavigator;
  fetcher?: AnalyticsFetcher;
  now?: () => Date;
};

export type AnalyticsClient = {
  trackLandingCtaClicked: (
    dimensions: LandingCtaClickedDimensions,
  ) => Promise<boolean>;
};

const requiredLandingCtaDimensions = [
  "variant_id",
  "product_line",
  "cta_id",
  "placement",
  "destination",
] as const satisfies ReadonlyArray<keyof LandingCtaClickedDimensions>;

export function createAnalyticsClient(
  options: AnalyticsClientOptions = {},
): AnalyticsClient {
  const endpoint = options.endpoint ?? DEFAULT_ANALYTICS_ENDPOINT;
  const enabled = options.enabled ?? true;
  const clock = options.now ?? (() => new Date());
  const navigatorRef =
    "navigator" in options ? options.navigator : globalThis.navigator;
  const fetcher =
    "fetcher" in options ? options.fetcher : getGlobalFetchOrUndefined();

  return {
    async trackLandingCtaClicked(dimensions) {
      if (!enabled || !isValidLandingCtaDimensions(dimensions)) {
        return false;
      }

      const payload: LandingCtaClickedPayload = {
        event_name: LANDING_ANALYTICS_EVENTS.landing_cta_clicked.event_name,
        schema_version:
          LANDING_ANALYTICS_EVENTS.landing_cta_clicked.schema_version,
        occurred_at: clock().toISOString(),
        dimensions,
      };

      return emitAnalyticsPayload({
        endpoint,
        payload,
        navigator: navigatorRef,
        fetcher,
      });
    },
  };
}

function isValidLandingCtaDimensions(
  dimensions: LandingCtaClickedDimensions,
): boolean {
  return requiredLandingCtaDimensions.every((dimension) =>
    hasNonEmptyString(dimensions[dimension]),
  );
}

function hasNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

async function emitAnalyticsPayload({
  endpoint,
  payload,
  navigator,
  fetcher,
}: {
  endpoint: string;
  payload: LandingCtaClickedPayload;
  navigator?: AnalyticsNavigator;
  fetcher?: AnalyticsFetcher;
}): Promise<boolean> {
  const body = JSON.stringify(payload);

  try {
    if (navigator?.sendBeacon?.(endpoint, body)) {
      return true;
    }

    if (!fetcher) {
      return false;
    }

    const response = await fetcher(endpoint, {
      method: "POST",
      keepalive: true,
      headers: {
        "content-type": "application/json",
      },
      body,
    });

    return response.ok;
  } catch {
    return false;
  }
}

function getGlobalFetchOrUndefined(): AnalyticsFetcher | undefined {
  if (typeof globalThis.fetch !== "function") {
    return undefined;
  }

  return (input, init) => globalThis.fetch(input, init);
}
