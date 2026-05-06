import { HttpClient, type ClientOptions } from "../runtime/fetch.js";
import { generateUuidV7 } from "../runtime/idempotency.js";
import { i64ToJsonNumber, parseI64, type JsonI64 } from "../runtime/i64.js";
import type {
  LedgerEntry,
  PostTransactionRequest,
  PostTransactionResponse,
  WalletBalance,
} from "./types.js";

interface ApiWalletBalance {
  user_id: string;
  available_minor_units: JsonI64;
  locked_minor_units: JsonI64;
  frozen_minor_units: JsonI64;
}

interface ApiLedgerEntry {
  txn_id: JsonI64;
  account_code: string;
  direction: "debit" | "credit";
  amount_minor_units: JsonI64;
  balance_after_minor_units: JsonI64;
}

interface ApiLedgerEntriesResponse {
  entries: ApiLedgerEntry[];
}

interface ApiPostTransactionResponse {
  txn_id: JsonI64;
  idempotency_key: string;
  replayed: boolean;
}

export interface WalletClient {
  getBalance(userId?: string): Promise<WalletBalance>;
  listEntries(accountCode: string, limit?: number): Promise<LedgerEntry[]>;
  postTransaction(request: PostTransactionRequest): Promise<PostTransactionResponse>;
}

export function createWalletClient(options: ClientOptions): WalletClient {
  const http = new HttpClient(options);

  return {
    async getBalance(userId?: string): Promise<WalletBalance> {
      const path =
        userId === undefined
          ? "/v1/me/wallet/balance"
          : `/internal/ledger/wallets/${encodeURIComponent(userId)}/balance`;
      return mapBalance(await http.json<ApiWalletBalance>({ path }));
    },

    async listEntries(accountCode: string, limit = 100): Promise<LedgerEntry[]> {
      const query = new URLSearchParams({
        account_code: accountCode,
        limit: String(limit),
      });
      const response = await http.json<ApiLedgerEntriesResponse>({
        path: "/v1/ledger/entries",
        query,
      });
      return response.entries.map(mapEntry);
    },

    async postTransaction(request: PostTransactionRequest): Promise<PostTransactionResponse> {
      const response = await http.json<ApiPostTransactionResponse>({
        method: "POST",
        path: "/internal/ledger/transactions",
        body: {
          txn_type: request.txnType,
          idempotency_key: request.idempotencyKey ?? generateUuidV7(),
          correlation_id: request.correlationId,
          description: request.description,
          entries: request.entries.map((entry) => ({
            account_code: entry.accountCode,
            direction: entry.direction,
            amount_minor_units: i64ToJsonNumber(entry.amountMinorUnits),
          })),
        },
      });

      return {
        txnId: parseI64(response.txn_id),
        idempotencyKey: response.idempotency_key,
        replayed: response.replayed,
      };
    },
  };
}

function mapBalance(balance: ApiWalletBalance): WalletBalance {
  return {
    userId: balance.user_id,
    availableMinorUnits: parseI64(balance.available_minor_units),
    lockedMinorUnits: parseI64(balance.locked_minor_units),
    frozenMinorUnits: parseI64(balance.frozen_minor_units),
  };
}

function mapEntry(entry: ApiLedgerEntry): LedgerEntry {
  return {
    txnId: parseI64(entry.txn_id),
    accountCode: entry.account_code,
    direction: entry.direction,
    amountMinorUnits: parseI64(entry.amount_minor_units),
    balanceAfterMinorUnits: parseI64(entry.balance_after_minor_units),
  };
}
