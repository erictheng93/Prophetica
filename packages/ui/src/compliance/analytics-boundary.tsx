"use client";

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react";

export interface AnalyticsEventPayload {
  eventName: string;
  dimensions: Record<string, string>;
}

export type AnalyticsTracker = (
  event: AnalyticsEventPayload,
) => void | Promise<void>;

export interface AnalyticsBoundaryProps {
  children: ReactNode;
  tracker?: AnalyticsTracker;
}

const AnalyticsContext = createContext<AnalyticsTracker | undefined>(undefined);

export function AnalyticsBoundary({
  children,
  tracker,
}: AnalyticsBoundaryProps) {
  const value = useMemo(() => tracker, [tracker]);

  return (
    <AnalyticsContext.Provider value={value}>
      {children}
    </AnalyticsContext.Provider>
  );
}

export function useAnalyticsTracker() {
  const tracker = useContext(AnalyticsContext);

  return useCallback(
    (event: AnalyticsEventPayload) => {
      void tracker?.(event);
    },
    [tracker],
  );
}
