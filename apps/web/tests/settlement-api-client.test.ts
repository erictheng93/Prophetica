import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchSettlementSummaries,
  submitMarketDispute,
} from "../lib/settlement-api.js";

test("submitMarketDispute posts SPEC-004 dispute payload", async () => {
  const calls: Array<{ url: string; body: unknown }> = [];
  const fetcher = async (input: string | URL, init?: RequestInit): Promise<Response> => {
    calls.push({
      url: String(input),
      body: JSON.parse(String(init?.body)),
    });
    return Response.json(
      {
        dispute_id: "disp_mkt_1",
        status: "PENDING_REVIEW",
        market_status: "disputed",
        deadline: "2026-06-02T00:00:00+08:00",
      },
      { status: 201 },
    );
  };

  const result = await submitMarketDispute(
    {
      marketId: "mkt-election",
      userId: "user_01",
      reason: "official source says NO",
      evidenceUrls: ["https://example.test/no"],
      claimedOutcome: "NO",
    },
    { baseUrl: "http://market.local", fetcher },
  );

  assert.equal(calls[0]?.url, "http://market.local/api/v1/markets/mkt-election/disputes");
  assert.deepEqual(calls[0]?.body, {
    user_id: "user_01",
    reason: "official source says NO",
    evidence_urls: ["https://example.test/no"],
    claimed_outcome: "NO",
  });
  assert.deepEqual(result, {
    disputeId: "disp_mkt_1",
    status: "PENDING_REVIEW",
    marketStatus: "disputed",
    deadline: "2026-06-02T00:00:00+08:00",
  });
});

test("fetchSettlementSummaries maps resolved markets from public settlement list", async () => {
  const fetcher = async (input: string | URL): Promise<Response> => {
    assert.equal(String(input), "http://market.local/api/v1/settlements?since=2026-06-01");
    return Response.json({
      settlements: [
        {
          market_id: "mkt-settled",
          title: "Settled market",
          status: "settled",
          resolved_outcome: "YES",
          resolved_at: "2026-06-02T00:00:00+08:00",
          oracle_source: "official result",
        },
      ],
    });
  };

  const result = await fetchSettlementSummaries({
    baseUrl: "http://market.local",
    fetcher,
    since: "2026-06-01",
  });

  assert.deepEqual(result, [
    {
      marketId: "mkt-settled",
      title: "Settled market",
      status: "settled",
      resolvedOutcome: "YES",
      resolvedAt: "2026-06-02T00:00:00+08:00",
      oracleSource: "official result",
    },
  ]);
});
