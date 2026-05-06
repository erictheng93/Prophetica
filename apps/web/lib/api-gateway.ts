export const API_GATEWAY_FALLBACK_USER_ID = "user_01";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_LEDGER_LIMIT = 25;

export interface WalletBalance {
  userId: string;
  availablePoints: number;
  lockedPoints: number;
  frozenPoints: number;
}

export interface LedgerEntry {
  txnId: number;
  accountCode: string;
  direction: "debit" | "credit";
  amountPoints: number;
  balanceAfterPoints: number;
}

export interface WalletOverview {
  balance: WalletBalance;
  ledgerEntries: LedgerEntry[];
  source: "api-gateway" | "fallback:api-gateway-unavailable";
}

interface ApiWalletBalance {
  user_id: string;
  available_minor_units: number;
  locked_minor_units: number;
  frozen_minor_units: number;
}

interface ApiLedgerEntry {
  txn_id: number;
  account_code: string;
  direction: "debit" | "credit";
  amount_minor_units: number;
  balance_after_minor_units: number;
}

interface ApiLedgerEntriesResponse {
  entries: ApiLedgerEntry[];
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface FetchWalletOverviewOptions {
  baseUrl?: string;
  userId?: string;
  fetcher?: Fetcher;
  ledgerLimit?: number;
}

export function minorUnitsToPoints(minorUnits: number): number {
  return -minorUnits / 100;
}

export async function fetchWalletOverview(
  options: FetchWalletOverviewOptions = {},
): Promise<WalletOverview> {
  const baseUrl = trimTrailingSlash(
    options.baseUrl ?? process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  );
  const userId = options.userId ?? process.env.PROPHETICA_DEV_USER_ID ?? API_GATEWAY_FALLBACK_USER_ID;
  const fetcher = options.fetcher ?? fetch;
  const ledgerLimit = options.ledgerLimit ?? DEFAULT_LEDGER_LIMIT;

  try {
    const [balance, ledgerEntries] = await Promise.all([
      fetchJson<ApiWalletBalance>(fetcher, `${baseUrl}/v1/me/wallet/balance`, userId),
      fetchJson<ApiLedgerEntriesResponse>(
        fetcher,
        `${baseUrl}/v1/ledger/entries?${new URLSearchParams({
          account_code: `Player.Wallet.${userId}`,
          limit: String(ledgerLimit),
        })}`,
        userId,
      ),
    ]);

    return {
      balance: mapWalletBalance(balance),
      ledgerEntries: ledgerEntries.entries.map(mapLedgerEntry),
      source: "api-gateway",
    };
  } catch {
    return fallbackWalletOverview(userId);
  }
}

async function fetchJson<T>(fetcher: Fetcher, url: string, userId: string): Promise<T> {
  const response = await fetcher(url, {
    cache: "no-store",
    headers: {
      "x-user-id": userId,
      accept: "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`api-gateway request failed: ${response.status}`);
  }

  return (await response.json()) as T;
}

function mapWalletBalance(balance: ApiWalletBalance): WalletBalance {
  return {
    userId: balance.user_id,
    availablePoints: minorUnitsToPoints(balance.available_minor_units),
    lockedPoints: minorUnitsToPoints(balance.locked_minor_units),
    frozenPoints: minorUnitsToPoints(balance.frozen_minor_units),
  };
}

function mapLedgerEntry(entry: ApiLedgerEntry): LedgerEntry {
  return {
    txnId: entry.txn_id,
    accountCode: entry.account_code,
    direction: entry.direction,
    amountPoints: minorUnitsToPoints(
      entry.direction === "credit" ? -entry.amount_minor_units : entry.amount_minor_units,
    ),
    balanceAfterPoints: minorUnitsToPoints(entry.balance_after_minor_units),
  };
}

function fallbackWalletOverview(userId: string): WalletOverview {
  return {
    balance: {
      userId,
      availablePoints: 0,
      lockedPoints: 0,
      frozenPoints: 0,
    },
    ledgerEntries: [],
    source: "fallback:api-gateway-unavailable",
  };
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
