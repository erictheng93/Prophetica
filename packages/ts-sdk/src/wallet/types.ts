import type { JsonI64 } from "../runtime/i64.js";

export type EntryDirection = "debit" | "credit";

export interface WalletBalance {
  userId: string;
  availableMinorUnits: bigint;
  lockedMinorUnits: bigint;
  frozenMinorUnits: bigint;
}

export interface LedgerEntry {
  txnId: bigint;
  accountCode: string;
  direction: EntryDirection;
  amountMinorUnits: bigint;
  balanceAfterMinorUnits: bigint;
}

export interface PostTransactionEntry {
  accountCode: string;
  direction: EntryDirection;
  amountMinorUnits: JsonI64;
}

export interface PostTransactionRequest {
  txnType: string;
  idempotencyKey?: string;
  correlationId?: string;
  description?: string;
  entries: PostTransactionEntry[];
}

export interface PostTransactionResponse {
  txnId: bigint;
  idempotencyKey: string;
  replayed: boolean;
}
