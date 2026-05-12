export interface ErrorEnvelope {
  code: string;
  message: string;
  traceId?: string;
  idempotencyKey?: string;
  details?: unknown;
}

export class ApiError extends Error {
  readonly status: number;
  readonly envelope: ErrorEnvelope;

  constructor(status: number, envelope: ErrorEnvelope) {
    super(envelope.message);
    this.name = "ApiError";
    this.status = status;
    this.envelope = envelope;
  }
}

export function decodeErrorEnvelope(status: number, body: unknown): ApiError {
  const record = isRecord(body) ? body : {};
  const code =
    readString(record.code) ?? readString(record.error) ?? `HTTP_${status}`;
  const message =
    readString(record.message) ??
    readString(record.error) ??
    `Request failed with status ${status}`;

  return new ApiError(status, {
    code,
    message,
    traceId: readString(record.trace_id) ?? readString(record.traceId),
    idempotencyKey:
      readString(record.idempotency_key) ?? readString(record.idempotencyKey),
    details: record.details,
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}
