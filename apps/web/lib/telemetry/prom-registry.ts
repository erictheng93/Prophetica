// Minimal in-process Prometheus exposition format registry.
//
// Why custom and not `prom-client`: avoids a new prod dep for the SPEC-005a
// scope, and keeps the exposition format under our control so later we can
// swap to OTLP / pushgateway without touching the call sites. Counter and
// Histogram cover everything SPEC-005a §7.3 lists.
//
// Label values are written verbatim — callers MUST sanitise to enum-bounded
// values before passing in (see kyc-metrics.ts). Prom text exposition has
// limited escape rules for label values: backslash, quote, newline.

export type MetricKind = "counter" | "histogram";

export interface CounterMetric {
  kind: "counter";
  name: string;
  help: string;
  labelNames: ReadonlyArray<string>;
  values: Map<string, number>;
}

export interface HistogramMetric {
  kind: "histogram";
  name: string;
  help: string;
  labelNames: ReadonlyArray<string>;
  buckets: ReadonlyArray<number>;
  // Per labelKey: bucketCounts[i] holds count <= buckets[i]; sum + count totals.
  observations: Map<
    string,
    { bucketCounts: number[]; sum: number; count: number }
  >;
}

export type Metric = CounterMetric | HistogramMetric;

export interface PromRegistry {
  counter(
    name: string,
    help: string,
    labelNames: ReadonlyArray<string>,
  ): CounterMetric;
  histogram(
    name: string,
    help: string,
    labelNames: ReadonlyArray<string>,
    buckets: ReadonlyArray<number>,
  ): HistogramMetric;
  inc(metric: CounterMetric, labels: Record<string, string>, by?: number): void;
  observe(
    metric: HistogramMetric,
    labels: Record<string, string>,
    value: number,
  ): void;
  /** Prometheus text exposition format (https://prometheus.io/docs/instrumenting/exposition_formats/). */
  exposition(): string;
  /** Test helper — wipes all values. Not used in prod. */
  reset(): void;
}

export function createPromRegistry(): PromRegistry {
  const metrics: Metric[] = [];

  function counter(
    name: string,
    help: string,
    labelNames: ReadonlyArray<string>,
  ): CounterMetric {
    const existing = metrics.find((m) => m.name === name);
    if (existing) {
      assertSameShape(existing, "counter", labelNames);
      return existing as CounterMetric;
    }
    const m: CounterMetric = {
      kind: "counter",
      name,
      help,
      labelNames,
      values: new Map(),
    };
    metrics.push(m);
    return m;
  }

  function histogram(
    name: string,
    help: string,
    labelNames: ReadonlyArray<string>,
    buckets: ReadonlyArray<number>,
  ): HistogramMetric {
    const existing = metrics.find((m) => m.name === name);
    if (existing) {
      assertSameShape(existing, "histogram", labelNames);
      return existing as HistogramMetric;
    }
    const m: HistogramMetric = {
      kind: "histogram",
      name,
      help,
      labelNames,
      buckets: [...buckets].sort((a, b) => a - b),
      observations: new Map(),
    };
    metrics.push(m);
    return m;
  }

  function inc(
    metric: CounterMetric,
    labels: Record<string, string>,
    by: number = 1,
  ): void {
    const key = labelKey(metric.labelNames, labels);
    metric.values.set(key, (metric.values.get(key) ?? 0) + by);
  }

  function observe(
    metric: HistogramMetric,
    labels: Record<string, string>,
    value: number,
  ): void {
    const key = labelKey(metric.labelNames, labels);
    let bucket = metric.observations.get(key);
    if (!bucket) {
      bucket = {
        bucketCounts: metric.buckets.map(() => 0),
        sum: 0,
        count: 0,
      };
      metric.observations.set(key, bucket);
    }
    bucket.count += 1;
    bucket.sum += value;
    for (let i = 0; i < metric.buckets.length; i += 1) {
      if (value <= metric.buckets[i]!) {
        bucket.bucketCounts[i]! += 1;
      }
    }
  }

  function exposition(): string {
    const out: string[] = [];
    for (const m of metrics) {
      out.push(`# HELP ${m.name} ${m.help}`);
      out.push(`# TYPE ${m.name} ${m.kind}`);
      if (m.kind === "counter") {
        for (const [key, val] of m.values) {
          out.push(`${m.name}${key} ${val}`);
        }
      } else {
        for (const [key, observation] of m.observations) {
          for (let i = 0; i < m.buckets.length; i += 1) {
            const bucketLabel = withExtraLabel(
              key,
              "le",
              formatBucketBound(m.buckets[i]!),
            );
            out.push(
              `${m.name}_bucket${bucketLabel} ${observation.bucketCounts[i]}`,
            );
          }
          out.push(
            `${m.name}_bucket${withExtraLabel(key, "le", "+Inf")} ${observation.count}`,
          );
          out.push(`${m.name}_sum${key} ${observation.sum}`);
          out.push(`${m.name}_count${key} ${observation.count}`);
        }
      }
    }
    out.push("");
    return out.join("\n");
  }

  function reset(): void {
    for (const m of metrics) {
      if (m.kind === "counter") {
        m.values.clear();
      } else {
        m.observations.clear();
      }
    }
  }

  return {
    counter,
    histogram,
    inc,
    observe,
    exposition,
    reset,
  };
}

function assertSameShape(
  metric: Metric,
  expectedKind: MetricKind,
  labelNames: ReadonlyArray<string>,
): void {
  if (metric.kind !== expectedKind) {
    throw new Error(
      `metric ${metric.name} re-registered as ${expectedKind} but exists as ${metric.kind}`,
    );
  }
  if (metric.labelNames.join(",") !== labelNames.join(",")) {
    throw new Error(
      `metric ${metric.name} re-registered with different label set`,
    );
  }
}

function labelKey(
  labelNames: ReadonlyArray<string>,
  labels: Record<string, string>,
): string {
  if (labelNames.length === 0) return "";
  const parts = labelNames.map(
    (name) => `${name}="${escapeLabelValue(labels[name] ?? "")}"`,
  );
  return `{${parts.join(",")}}`;
}

function withExtraLabel(
  key: string,
  extraName: string,
  extraValue: string,
): string {
  const escaped = escapeLabelValue(extraValue);
  if (key === "") {
    return `{${extraName}="${escaped}"}`;
  }
  // Insert before the closing brace.
  return `${key.slice(0, -1)},${extraName}="${escaped}"}`;
}

function escapeLabelValue(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/\n/g, "\\n").replace(/"/g, '\\"');
}

function formatBucketBound(bound: number): string {
  if (!Number.isFinite(bound)) return "+Inf";
  return Number.isInteger(bound) ? bound.toString() : bound.toString();
}
