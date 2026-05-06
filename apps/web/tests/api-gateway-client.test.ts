import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { test } from "node:test";

import {
  API_GATEWAY_FALLBACK_USER_ID,
  fetchWalletOverview,
  minorUnitsToPoints,
  type WalletClientFactory,
} from "../lib/api-gateway.js";

test("minorUnitsToPoints converts backend minor units into display points", () => {
  assert.equal(minorUnitsToPoints(-125_000), 1_250);
  assert.equal(minorUnitsToPoints(25_050), -250.5);
});

test("web declares ts-sdk as the wallet client dependency", async () => {
  const packageJson = JSON.parse(await readFile(join(process.cwd(), "package.json"), "utf8")) as {
    dependencies?: Record<string, string>;
  };

  assert.equal(packageJson.dependencies?.["@prophetica/ts-sdk"], "workspace:*");
});

test("fetchWalletOverview calls api-gateway wallet and ledger endpoints with auth context", async () => {
  const calls: Array<{ url: string; headers: Record<string, string> }> = [];
  const fetcher = async (input: string | URL, init?: RequestInit): Promise<Response> => {
    calls.push({
      url: String(input),
      headers: Object.fromEntries(new Headers(init?.headers).entries()),
    });

    if (String(input).includes("/v1/me/wallet/balance")) {
      return Response.json({
        user_id: "user_42",
        available_minor_units: -125_000,
        locked_minor_units: -25_000,
        frozen_minor_units: 0,
      });
    }

    return Response.json({
      entries: [
        {
          txn_id: 7,
          account_code: "Player.Wallet.user_42",
          direction: "credit",
          amount_minor_units: 50_000,
          balance_after_minor_units: -125_000,
        },
      ],
    });
  };

  const overview = await fetchWalletOverview({
    baseUrl: "http://api.local",
    userId: "user_42",
    fetcher,
    walletClientFactory: createFetchBackedWalletClient,
  });

  assert.equal(overview.balance.availablePoints, 1_250);
  assert.equal(overview.balance.lockedPoints, 250);
  assert.equal(overview.ledgerEntries[0]?.amountPoints, 500);
  assert.equal(calls[0]?.url, "http://api.local/v1/me/wallet/balance");
  assert.equal(
    calls[1]?.url,
    "http://api.local/v1/ledger/entries?account_code=Player.Wallet.user_42&limit=25",
  );
  assert.equal(calls[0]?.headers["x-user-id"], "user_42");
  assert.equal(calls[1]?.headers["x-user-id"], "user_42");
});

test("fetchWalletOverview returns a safe fallback when api-gateway is unavailable", async () => {
  const overview = await fetchWalletOverview({
    baseUrl: "http://api.local",
    userId: API_GATEWAY_FALLBACK_USER_ID,
    fetcher: async () => {
      throw new Error("connection refused");
    },
    walletClientFactory: createFetchBackedWalletClient,
  });

  assert.equal(overview.balance.userId, API_GATEWAY_FALLBACK_USER_ID);
  assert.equal(overview.balance.availablePoints, 0);
  assert.equal(overview.ledgerEntries.length, 0);
  assert.match(overview.source, /fallback/);
});

const createFetchBackedWalletClient: WalletClientFactory = ({ baseUrl, auth, fetcher = fetch }) => ({
  async getBalance() {
    const response = await fetcher(`${baseUrl}/v1/me/wallet/balance`, {
      cache: "no-store",
      headers: {
        accept: "application/json",
        "x-user-id": auth?.userId ?? "",
      },
    });
    const balance = (await response.json()) as {
      user_id: string;
      available_minor_units: number;
      locked_minor_units: number;
      frozen_minor_units: number;
    };
    return {
      userId: balance.user_id,
      availableMinorUnits: BigInt(balance.available_minor_units),
      lockedMinorUnits: BigInt(balance.locked_minor_units),
      frozenMinorUnits: BigInt(balance.frozen_minor_units),
    };
  },

  async listEntries(accountCode, limit = 100) {
    const response = await fetcher(
      `${baseUrl}/v1/ledger/entries?${new URLSearchParams({
        account_code: accountCode,
        limit: String(limit),
      })}`,
      {
        cache: "no-store",
        headers: {
          accept: "application/json",
          "x-user-id": auth?.userId ?? "",
        },
      },
    );
    const payload = (await response.json()) as {
      entries: Array<{
        txn_id: number;
        account_code: string;
        direction: "debit" | "credit";
        amount_minor_units: number;
        balance_after_minor_units: number;
      }>;
    };
    return payload.entries.map((entry) => ({
      txnId: BigInt(entry.txn_id),
      accountCode: entry.account_code,
      direction: entry.direction,
      amountMinorUnits: BigInt(entry.amount_minor_units),
      balanceAfterMinorUnits: BigInt(entry.balance_after_minor_units),
    }));
  },

  async postTransaction() {
    throw new Error("postTransaction is not used by fetchWalletOverview");
  },
});
