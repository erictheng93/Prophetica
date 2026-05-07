// Frontend structured logger — SPEC-007 §4.1 (JSON shape) + §4.3 (PII rule).
//
// Allowlist over denylist: payload fields are filtered through `SAFE_KEYS`.
// Anything not on the list is dropped. This is safer than regex denylists
// because new fields default to "not collected" rather than "collected then
// hopefully redacted". See SPEC-007 review note D-Q3.

export type LogLevel = "debug" | "info" | "warn" | "error" | "critical";

const LEVEL_RANK: Record<LogLevel, number> = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40,
  critical: 50,
};

// Keys that are always safe to emit. Domain identifiers only — no PII.
// Adding a key here is a deliberate decision; review against SPEC-007 §4.3.
export const SAFE_KEYS: ReadonlySet<string> = new Set([
  "event",
  "msg",
  "service",
  "version",
  "trace_id",
  "span_id",
  "parent_span_id",
  "user_id",
  "market_id",
  "order_id",
  "txn_id",
  "duration_ms",
  "hop",
  "budget_ms",
  "status",
  "status_code",
  "method",
  "url_path",
  "source",
  "outcome",
  "reason_code",
]);

// Keys that look safe but historically leaked PII upstream — explicit deny.
const HARD_DENY_KEYS: ReadonlySet<string> = new Set([
  "name",
  "full_name",
  "first_name",
  "last_name",
  "email",
  "phone",
  "phone_number",
  "msisdn",
  "id_number",
  "national_id",
  "passport",
  "card_number",
  "pan",
  "iban",
  "address",
  "ip",
  "ip_address",
  "user_agent",
  "password",
  "token",
  "authorization",
  "cookie",
]);

// Last-resort string scrubber for free-form `msg`. Catches obvious shapes
// even though `msg` should already be a constant template, not interpolated PII.
const PII_SHAPE_PATTERNS: ReadonlyArray<{ re: RegExp; mask: string }> = [
  { re: /[\w.+-]+@[\w-]+\.[\w.-]+/g, mask: "[email]" },
  { re: /\b\d{13,19}\b/g, mask: "[card]" },
  { re: /\b09\d{8}\b/g, mask: "[phone]" },
  { re: /\b\+?\d{1,3}[-\s]?\(?\d{2,4}\)?[-\s]?\d{3,4}[-\s]?\d{3,4}\b/g, mask: "[phone]" },
  { re: /\b[A-Z][12]\d{8}\b/g, mask: "[twid]" },
];

export interface LoggerConfig {
  service: string;
  version: string;
  minLevel?: LogLevel;
  // sink defaults to console; tests inject an in-memory sink.
  sink?: (record: LogRecord) => void;
  // Override clock for deterministic tests.
  now?: () => Date;
}

export interface LogRecord {
  timestamp: string;
  level: LogLevel;
  service: string;
  version: string;
  event: string;
  msg: string;
  // All other safe fields are inlined at top level (matches SPEC §4.1 sample).
  [k: string]: unknown;
}

export type LogPayload = Record<string, unknown>;

export interface Logger {
  debug(event: string, msg: string, payload?: LogPayload): void;
  info(event: string, msg: string, payload?: LogPayload): void;
  warn(event: string, msg: string, payload?: LogPayload): void;
  error(event: string, msg: string, payload?: LogPayload): void;
  critical(event: string, msg: string, payload?: LogPayload): void;
}

export function createLogger(config: LoggerConfig): Logger {
  const minLevel = config.minLevel ?? "info";
  const sink = config.sink ?? defaultConsoleSink;
  const now = config.now ?? (() => new Date());

  function emit(level: LogLevel, event: string, msg: string, payload?: LogPayload) {
    if (LEVEL_RANK[level] < LEVEL_RANK[minLevel]) return;
    const safe = filterPayload(payload);
    const record: LogRecord = {
      timestamp: now().toISOString(),
      level,
      service: config.service,
      version: config.version,
      event,
      msg: scrubMessageShape(msg),
      ...safe,
    };
    sink(record);
  }

  return {
    debug: (e, m, p) => emit("debug", e, m, p),
    info: (e, m, p) => emit("info", e, m, p),
    warn: (e, m, p) => emit("warn", e, m, p),
    error: (e, m, p) => emit("error", e, m, p),
    critical: (e, m, p) => emit("critical", e, m, p),
  };
}

function filterPayload(payload?: LogPayload): LogPayload {
  if (!payload) return {};
  const out: LogPayload = {};
  for (const [key, value] of Object.entries(payload)) {
    if (HARD_DENY_KEYS.has(key)) continue;
    if (!SAFE_KEYS.has(key)) continue;
    out[key] = sanitizeValue(value);
  }
  return out;
}

function sanitizeValue(value: unknown): unknown {
  if (value == null) return value;
  if (typeof value === "string") return scrubMessageShape(value);
  if (typeof value === "number" || typeof value === "boolean") return value;
  if (typeof value === "bigint") return value.toString();
  // Reject objects/arrays from payload — encourages flat schema (§4.1).
  return "[unsupported]";
}

function scrubMessageShape(msg: string): string {
  let out = msg;
  for (const { re, mask } of PII_SHAPE_PATTERNS) out = out.replace(re, mask);
  return out;
}

// Re-exported for test inspection only — confirms the deny list isn't empty
// and contains the expected high-risk keys.
export const HARD_DENY_KEYS_FOR_TEST = HARD_DENY_KEYS;

function defaultConsoleSink(record: LogRecord): void {
  if (typeof console === "undefined") return;
  const line = JSON.stringify(record);
  switch (record.level) {
    case "debug":
      console.debug(line);
      break;
    case "info":
      console.info(line);
      break;
    case "warn":
      console.warn(line);
      break;
    case "error":
    case "critical":
      console.error(line);
      break;
  }
}
