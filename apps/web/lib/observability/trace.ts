// W3C Trace Context — SPEC-007 §5.1.
//
// Browser/mobile clients are the trace ROOT. We generate traceparent here so
// api-gateway can adopt it as its root span. (See SPEC-007 review note A2.)
//
// Format (W3C Trace Context level 1):
//   traceparent: <version>-<trace-id 16B hex>-<parent-id 8B hex>-<flags 1B hex>
// We always emit version=00 and flags=01 (sampled=true). Sampling decision is
// a head-side hint only; OTel Collector applies tail-based sampling per
// SPEC-007 review note B4.

const VERSION = "00";
const FLAG_SAMPLED = "01";
const TRACE_ID_BYTES = 16;
const SPAN_ID_BYTES = 8;
const TRACEPARENT_RE =
  /^([0-9a-f]{2})-([0-9a-f]{32})-([0-9a-f]{16})-([0-9a-f]{2})$/i;

export interface TraceContext {
  traceId: string;
  spanId: string;
  parentSpanId?: string;
  sampled: boolean;
}

export function newTraceContext(parent?: TraceContext): TraceContext {
  return {
    traceId: parent?.traceId ?? randomHex(TRACE_ID_BYTES),
    spanId: randomHex(SPAN_ID_BYTES),
    parentSpanId: parent?.spanId,
    sampled: true,
  };
}

export function formatTraceparent(ctx: TraceContext): string {
  return `${VERSION}-${ctx.traceId}-${ctx.spanId}-${ctx.sampled ? FLAG_SAMPLED : "00"}`;
}

export function parseTraceparent(header: string): TraceContext | null {
  const match = TRACEPARENT_RE.exec(header.trim());
  if (!match) return null;
  const version = match[1];
  const traceId = match[2];
  const spanId = match[3];
  const flags = match[4];
  if (!version || !traceId || !spanId || !flags) return null;
  if (version !== VERSION) return null;
  if (traceId === "0".repeat(32) || spanId === "0".repeat(16)) return null;
  return {
    traceId: traceId.toLowerCase(),
    spanId: spanId.toLowerCase(),
    sampled: (parseInt(flags, 16) & 0x01) === 0x01,
  };
}

export type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface TracedFetchOptions {
  baseFetch?: Fetcher;
  context?: TraceContext;
}

export function tracedFetch(
  input: string | URL,
  init?: RequestInit,
  options: TracedFetchOptions = {},
): Promise<Response> {
  const baseFetch = options.baseFetch ?? fetch;
  const ctx = options.context ?? newTraceContext();
  const headers = new Headers(init?.headers);
  if (!headers.has("traceparent")) {
    headers.set("traceparent", formatTraceparent(ctx));
  }
  return baseFetch(input, { ...init, headers });
}

function randomHex(byteLen: number): string {
  const bytes = new Uint8Array(byteLen);
  getCrypto().getRandomValues(bytes);
  let out = "";
  for (const byte of bytes) out += byte.toString(16).padStart(2, "0");
  return out;
}

interface RandomSource {
  getRandomValues(array: Uint8Array): Uint8Array;
}

function getCrypto(): RandomSource {
  const c = (globalThis as { crypto?: RandomSource }).crypto;
  if (!c?.getRandomValues) {
    throw new Error("trace.ts: Web Crypto getRandomValues not available");
  }
  return c;
}
