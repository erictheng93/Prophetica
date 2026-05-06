const DEFAULT_MARKET_SERVICE_BASE_URL = "http://127.0.0.1:8087";

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

interface SettlementApiOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
}

export interface SubmitMarketDisputeInput {
  marketId: string;
  userId: string;
  reason: string;
  evidenceUrls: string[];
  claimedOutcome: "YES" | "NO" | "VOID";
}

interface ApiDisputeResponse {
  dispute_id: string;
  status: string;
  market_status: string;
  deadline: string;
}

export interface MarketDisputeResult {
  disputeId: string;
  status: string;
  marketStatus: string;
  deadline: string;
}

interface FetchSettlementOptions extends SettlementApiOptions {
  since?: string;
}

interface ApiSettlement {
  market_id: string;
  title: string;
  status: string;
  resolved_outcome: string | null;
  resolved_at: string | null;
  oracle_source: string;
}

interface ApiSettlementList {
  settlements: ApiSettlement[];
}

export interface SettlementSummary {
  marketId: string;
  title: string;
  status: string;
  resolvedOutcome: string | null;
  resolvedAt: string | null;
  oracleSource: string;
}

export async function submitMarketDispute(
  input: SubmitMarketDisputeInput,
  {
    baseUrl = process.env.MARKET_SERVICE_BASE_URL ?? DEFAULT_MARKET_SERVICE_BASE_URL,
    fetcher = fetch,
  }: SettlementApiOptions = {},
): Promise<MarketDisputeResult> {
  const url = new URL(
    `/api/v1/markets/${encodeURIComponent(input.marketId)}/disputes`,
    trimTrailingSlash(baseUrl) + "/",
  );
  const response = await fetcher(url, {
    method: "POST",
    cache: "no-store",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      user_id: input.userId,
      reason: input.reason,
      evidence_urls: input.evidenceUrls,
      claimed_outcome: input.claimedOutcome,
    }),
  });
  if (!response.ok) {
    throw new Error(`market-service dispute request failed: ${response.status}`);
  }

  const body = (await response.json()) as ApiDisputeResponse;
  return {
    disputeId: body.dispute_id,
    status: body.status,
    marketStatus: body.market_status,
    deadline: body.deadline,
  };
}

export async function fetchSettlementSummaries({
  baseUrl = process.env.MARKET_SERVICE_BASE_URL ?? DEFAULT_MARKET_SERVICE_BASE_URL,
  fetcher = fetch,
  since,
}: FetchSettlementOptions = {}): Promise<SettlementSummary[]> {
  const url = new URL("/api/v1/settlements", trimTrailingSlash(baseUrl) + "/");
  if (since) {
    url.searchParams.set("since", since);
  }

  const response = await fetcher(url, {
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`market-service settlement request failed: ${response.status}`);
  }

  const body = (await response.json()) as ApiSettlementList;
  return body.settlements.map((settlement) => ({
    marketId: settlement.market_id,
    title: settlement.title,
    status: settlement.status,
    resolvedOutcome: settlement.resolved_outcome,
    resolvedAt: settlement.resolved_at,
    oracleSource: settlement.oracle_source,
  }));
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
