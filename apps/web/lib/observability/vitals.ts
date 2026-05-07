// Web Vitals capture — supports SPEC-007 §6.4.3 client-receive budget by
// providing the LCP/INP/CLS evidence that "client side is not the bottleneck".
//
// Implemented with the native PerformanceObserver API rather than the
// `web-vitals` npm package so the bundle stays minimal (Cloudflare Pages free
// tier counts every KB on cold start). The API surface intentionally mirrors
// the relevant subset of `web-vitals` so we can swap implementations later
// without touching call sites.
//
// MVP: when no beacon endpoint is configured, capture is a no-op. Wiring the
// beacon to a Worker route is a follow-up SPEC item (see review note A1).

export type VitalName = "LCP" | "INP" | "CLS" | "TTFB";

export interface VitalSample {
  name: VitalName;
  value: number;
  rating: "good" | "needs-improvement" | "poor";
}

// Thresholds from web.dev/vitals (Core Web Vitals 2024).
const THRESHOLDS: Record<VitalName, { good: number; poor: number }> = {
  LCP: { good: 2500, poor: 4000 },
  INP: { good: 200, poor: 500 },
  CLS: { good: 0.1, poor: 0.25 },
  TTFB: { good: 800, poor: 1800 },
};

export function rate(name: VitalName, value: number): VitalSample["rating"] {
  const t = THRESHOLDS[name];
  if (value <= t.good) return "good";
  if (value <= t.poor) return "needs-improvement";
  return "poor";
}

export interface ObserveVitalsOptions {
  onSample: (sample: VitalSample) => void;
}

export function observeVitals({ onSample }: ObserveVitalsOptions): () => void {
  if (typeof PerformanceObserver === "undefined") return () => {};

  const observers: PerformanceObserver[] = [];

  const lcpObs = safeObserve("largest-contentful-paint", (entries) => {
    const last = entries[entries.length - 1];
    if (!last) return;
    const value = (last as PerformanceEntry & { renderTime?: number }).renderTime ?? last.startTime;
    onSample({ name: "LCP", value, rating: rate("LCP", value) });
  });
  if (lcpObs) observers.push(lcpObs);

  const clsObs = safeObserve("layout-shift", (entries) => {
    let value = 0;
    for (const entry of entries) {
      const ls = entry as PerformanceEntry & { value?: number; hadRecentInput?: boolean };
      if (ls.hadRecentInput) continue;
      value += ls.value ?? 0;
    }
    if (value > 0) onSample({ name: "CLS", value, rating: rate("CLS", value) });
  });
  if (clsObs) observers.push(clsObs);

  const inpObs = safeObserve("event", (entries) => {
    let worst = 0;
    for (const entry of entries) {
      if (entry.duration > worst) worst = entry.duration;
    }
    if (worst > 0) onSample({ name: "INP", value: worst, rating: rate("INP", worst) });
  });
  if (inpObs) observers.push(inpObs);

  const navTtfb = readTtfb();
  if (navTtfb !== null) {
    onSample({ name: "TTFB", value: navTtfb, rating: rate("TTFB", navTtfb) });
  }

  return () => {
    for (const o of observers) o.disconnect();
  };
}

function readTtfb(): number | null {
  if (typeof performance === "undefined" || !performance.getEntriesByType) return null;
  const nav = performance.getEntriesByType("navigation")[0] as
    | (PerformanceEntry & { responseStart?: number })
    | undefined;
  if (!nav?.responseStart) return null;
  return nav.responseStart;
}

function safeObserve(
  type: string,
  cb: (entries: PerformanceEntry[]) => void,
): PerformanceObserver | null {
  try {
    const observer = new PerformanceObserver((list) => cb(list.getEntries()));
    observer.observe({ type, buffered: true });
    return observer;
  } catch {
    return null;
  }
}

// Beacon: schedule a single payload via sendBeacon when the page hides. The
// beacon is fire-and-forget — never blocks navigation, never returns errors.
// If endpoint is unset, this entire path is a no-op.
export interface BeaconConfig {
  endpoint?: string;
  buildPayload: () => Record<string, unknown>;
}

export function installBeacon(config: BeaconConfig): () => void {
  if (typeof document === "undefined" || typeof navigator === "undefined") return () => {};
  if (!config.endpoint) return () => {};

  const handler = () => {
    if (document.visibilityState !== "hidden") return;
    const payload = config.buildPayload();
    if (!navigator.sendBeacon) return;
    const body = new Blob([JSON.stringify(payload)], { type: "application/json" });
    navigator.sendBeacon(config.endpoint!, body);
  };
  document.addEventListener("visibilitychange", handler, { passive: true });
  return () => document.removeEventListener("visibilitychange", handler);
}
