"use client";

import {
  LANDING_ANALYTICS_EVENTS,
  createAnalyticsClient,
  type LandingCtaClickedDimensions,
} from "@prophetica/ts-sdk/analytics";
import {
  AnalyticsBoundary,
  KycEntryButton,
  useAnalyticsTracker,
  type KycEntryButtonProps,
} from "@prophetica/ui/compliance";
import { useMemo, type ReactNode } from "react";

const landingAnalyticsClient = createAnalyticsClient();
const signupDestination = "/auth/sign-in?mode=signup";

export function LandingAnalyticsBoundary({
  children,
}: {
  children: ReactNode;
}) {
  const tracker = useMemo(
    () =>
      async ({
        eventName,
        dimensions,
      }: {
        eventName: string;
        dimensions: Record<string, string>;
      }) => {
        if (
          eventName !== LANDING_ANALYTICS_EVENTS.landing_cta_clicked.event_name
        ) {
          return;
        }

        await landingAnalyticsClient.trackLandingCtaClicked(
          dimensions as LandingCtaClickedDimensions,
        );
      },
    [],
  );

  return <AnalyticsBoundary tracker={tracker}>{children}</AnalyticsBoundary>;
}

export type TrackedSignupButtonProps = Omit<KycEntryButtonProps, "href"> & {
  placement: string;
};

export function TrackedSignupButton({
  placement,
  onClick,
  ...props
}: TrackedSignupButtonProps) {
  const track = useAnalyticsTracker();

  return (
    <KycEntryButton
      {...props}
      href={signupDestination}
      onClick={(event) => {
        track({
          eventName: LANDING_ANALYTICS_EVENTS.landing_cta_clicked.event_name,
          dimensions: {
            variant_id: "sports",
            product_line: "sports",
            region: "TW",
            cta_id: "signup",
            placement,
            destination: signupDestination,
          },
        });
        onClick?.(event);
      }}
    />
  );
}
