// Client-side latency budget instrumentation — SPEC-007 §6.4.
//
// SPEC-007 §6.4.2 lists per-hop P99 budgets. The frontend owns three hops:
//   - "client.submit"   : user click → tracedFetch first byte sent
//   - "client.receive"  : server first byte → in-app event-loop callback
//   - "client.render"   : callback → next paint (used by trade-panel UX SLA)
//
// We classify a measured duration against budget so callers can:
//   (1) attach `hop` + `budget_ms` to the structured log line, and
//   (2) decide whether to surface a perf-regression warning in dev tools.
//
// Numbers below match SPEC-007 §6.4.2 / §6.4.3 client-controllable rows. Any
// change requires the SPEC change-control process (§6.4.5) — do not adjust to
// silence a noisy alert.

export type Hop = "client.submit" | "client.receive" | "client.render";

export type BudgetStatus = "ok" | "warn" | "breach" | "p0";

const BUDGETS_MS: Record<Hop, number> = {
  "client.submit": 5,
  "client.receive": 10,
  "client.render": 16,
};

export function budgetMs(hop: Hop): number {
  return BUDGETS_MS[hop];
}

// Mirrors §6.4.4 thresholds: > 1.5x → P1 warn, > 2x → P0 breach.
export function classifyBudget(hop: Hop, durationMs: number): BudgetStatus {
  const budget = BUDGETS_MS[hop];
  if (durationMs <= budget) return "ok";
  if (durationMs <= budget * 1.5) return "warn";
  if (durationMs <= budget * 2) return "breach";
  return "p0";
}

export interface Measurement {
  hop: Hop;
  durationMs: number;
  budgetMs: number;
  status: BudgetStatus;
}

export interface Clock {
  now(): number;
}

export const performanceClock: Clock = {
  now: () => (typeof performance !== "undefined" ? performance.now() : Date.now()),
};

export function startMeasure(hop: Hop, clock: Clock = performanceClock): () => Measurement {
  const start = clock.now();
  return () => {
    const durationMs = clock.now() - start;
    return {
      hop,
      durationMs,
      budgetMs: BUDGETS_MS[hop],
      status: classifyBudget(hop, durationMs),
    };
  };
}

export async function withMeasure<T>(
  hop: Hop,
  fn: () => Promise<T>,
  options: { clock?: Clock; onComplete?: (m: Measurement) => void } = {},
): Promise<T> {
  const stop = startMeasure(hop, options.clock ?? performanceClock);
  try {
    return await fn();
  } finally {
    options.onComplete?.(stop());
  }
}
