export const ANALYTICS_SCHEMA_VERSION = "analytics.v1" as const;

export const LANDING_ANALYTICS_EVENTS = {
  landing_cta_clicked: {
    event_name: "landing_cta_clicked",
    schema_version: ANALYTICS_SCHEMA_VERSION,
    dimensions: [
      "variant_id",
      "product_line",
      "region",
      "cta_id",
      "placement",
      "destination",
    ],
  },
} as const;

export type LandingAnalyticsEventName = keyof typeof LANDING_ANALYTICS_EVENTS;

export type LandingCtaClickedDimensions = {
  variant_id: string;
  product_line: string;
  region?: string;
  cta_id: string;
  placement: string;
  destination: string;
};

export type LandingCtaClickedPayload = {
  event_name: typeof LANDING_ANALYTICS_EVENTS.landing_cta_clicked.event_name;
  schema_version: typeof ANALYTICS_SCHEMA_VERSION;
  occurred_at: string;
  dimensions: LandingCtaClickedDimensions;
};
