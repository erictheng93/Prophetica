export type ManualPointDirection = "grant" | "deduct";

export interface WalletBalance {
  user_id: string;
  available_minor_units: number;
  locked_minor_units: number;
  frozen_minor_units: number;
}

export interface ManualPointsAdjustmentRequest {
  target_user_id: string;
  direction: ManualPointDirection;
  delta_minor_units: number;
  reason_code: string;
  reason_text: string;
  correlation_id?: string;
  idempotency_key?: string;
}

export interface ManualPointsAdjustmentResponse {
  txn_id: number;
  idempotency_key: string;
  replayed: boolean;
  audit_log_id?: string | null;
}

export function parsePointAmountToMinorUnits(input: string): number {
  const trimmed = input.trim();
  const match = /^(\d+)(?:\.(\d{1,2}))?$/.exec(trimmed);
  if (!match) {
    throw new Error(
      "points amount must be a positive number with at most two decimals",
    );
  }

  const major = BigInt(match[1] ?? "0");
  const centsText = (match[2] ?? "").padEnd(2, "0");
  const minor = major * 100n + BigInt(centsText === "" ? "0" : centsText);
  if (minor <= 0n) {
    throw new Error("points amount must be greater than zero");
  }
  if (minor > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("points amount exceeds safe integer range");
  }

  return Number(minor);
}

export function formatMinorUnitsAsPoints(
  minorUnits: number,
  options: { liabilityBalance?: boolean } = {},
): string {
  const normalized = options.liabilityBalance ? -minorUnits : minorUnits;
  const sign = normalized < 0 ? "-" : "";
  const absolute = Math.abs(normalized);
  const major = Math.floor(absolute / 100);
  const minor = String(absolute % 100).padStart(2, "0");
  return `${sign}${major.toLocaleString("zh-Hant")}.${minor} 點`;
}

export function isManualPointDirection(
  value: unknown,
): value is ManualPointDirection {
  return value === "grant" || value === "deduct";
}
