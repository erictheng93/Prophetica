/*
 * Idempotency-Key generator + helper — SPEC-008 §7.2 client contract.
 *
 * UUID v7 (RFC 9562) chosen over v4 because the embedded 48-bit unix-ms
 * timestamp is operationally invaluable: backend audit logs, Kafka tail,
 * and OTel span attribute `idempotency.key` (SPEC-007 §6.4) can all be
 * cross-referenced by sorting on the same key. Same key reused across
 * retries — see SPEC-002 §6.1 for backend deduplication contract.
 */

const HEX_BYTE = (n: number) => n.toString(16).padStart(2, "0");

export function generateIdempotencyKey(now: number = Date.now()): string {
  const bytes = new Uint8Array(16);

  bytes[0] = (now / 2 ** 40) & 0xff;
  bytes[1] = (now / 2 ** 32) & 0xff;
  bytes[2] = (now >>> 24) & 0xff;
  bytes[3] = (now >>> 16) & 0xff;
  bytes[4] = (now >>> 8) & 0xff;
  bytes[5] = now & 0xff;

  const random = new Uint8Array(10);
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    globalThis.crypto.getRandomValues(random);
  } else {
    for (let i = 0; i < random.length; i += 1) {
      random[i] = Math.floor(Math.random() * 256);
    }
  }
  bytes.set(random, 6);

  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x70;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;

  const hex = Array.from(bytes, HEX_BYTE).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

export const IDEMPOTENCY_HEADER = "Idempotency-Key";

export interface IdempotentInit extends RequestInit {
  idempotencyKey?: string;
}

export function withIdempotencyKey(init: IdempotentInit = {}): RequestInit & {
  idempotencyKey: string;
} {
  const idempotencyKey = init.idempotencyKey ?? generateIdempotencyKey();
  const headers = new Headers(init.headers);
  headers.set(IDEMPOTENCY_HEADER, idempotencyKey);

  const { idempotencyKey: _omit, ...rest } = init;
  void _omit;

  return { ...rest, headers, idempotencyKey };
}

export function uuidV7Timestamp(key: string): number | null {
  const compact = key.replace(/-/g, "");
  if (compact.length !== 32) return null;
  const versionNibble = compact.charAt(12);
  if (versionNibble === "" || parseInt(versionNibble, 16) !== 0x7) return null;

  const high = parseInt(compact.slice(0, 6), 16);
  const low = parseInt(compact.slice(6, 12), 16);
  return high * 2 ** 24 + low;
}
