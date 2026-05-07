import type {
  ClientOptions,
  LedgerEntry as SdkLedgerEntry,
  WalletBalance as SdkWalletBalance,
  WalletClient,
} from "@prophetica/ts-sdk";

import { tracedFetch } from "./observability/trace.js";

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

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;
export type WalletClientFactory = (options: ClientOptions) => WalletClient;

export interface FetchWalletOverviewOptions {
  baseUrl?: string;
  userId?: string;
  fetcher?: Fetcher;
  ledgerLimit?: number;
  walletClientFactory?: WalletClientFactory;
}

export function minorUnitsToPoints(minorUnits: number | bigint): number {
  return -Number(minorUnits) / 100;
}

export async function fetchWalletOverview(
  options: FetchWalletOverviewOptions = {},
): Promise<WalletOverview> {
  const baseUrl = trimTrailingSlash(
    options.baseUrl ?? process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  );
  const userId = options.userId ?? process.env.PROPHETICA_DEV_USER_ID ?? API_GATEWAY_FALLBACK_USER_ID;
  // Default fetcher injects W3C traceparent (SPEC-007 §5.1). Tests pass an
  // explicit fetcher and bypass propagation by design.
  const fetcher = options.fetcher ?? ((input, init) => tracedFetch(input, init));
  const ledgerLimit = options.ledgerLimit ?? DEFAULT_LEDGER_LIMIT;
  const walletClientFactory = options.walletClientFactory ?? (await loadWalletClientFactory());
  const walletClient = walletClientFactory({
    baseUrl,
    auth: { userId },
    fetcher,
  });

  try {
    const [balance, ledgerEntries] = await Promise.all([
      walletClient.getBalance(),
      walletClient.listEntries(`Player.Wallet.${userId}`, ledgerLimit),
    ]);

    return {
      balance: mapWalletBalance(balance),
      ledgerEntries: ledgerEntries.map(mapLedgerEntry),
      source: "api-gateway",
    };
  } catch {
    return fallbackWalletOverview(userId);
  }
}

async function loadWalletClientFactory(): Promise<WalletClientFactory> {
  const { createWalletClient } = await import("@prophetica/ts-sdk");
  return createWalletClient;
}

function mapWalletBalance(balance: SdkWalletBalance): WalletBalance {
  return {
    userId: balance.userId,
    availablePoints: minorUnitsToPoints(balance.availableMinorUnits),
    lockedPoints: minorUnitsToPoints(balance.lockedMinorUnits),
    frozenPoints: minorUnitsToPoints(balance.frozenMinorUnits),
  };
}

function mapLedgerEntry(entry: SdkLedgerEntry): LedgerEntry {
  return {
    txnId: Number(entry.txnId),
    accountCode: entry.accountCode,
    direction: entry.direction,
    amountPoints: minorUnitsToPoints(
      entry.direction === "credit" ? -entry.amountMinorUnits : entry.amountMinorUnits,
    ),
    balanceAfterPoints: minorUnitsToPoints(entry.balanceAfterMinorUnits),
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
