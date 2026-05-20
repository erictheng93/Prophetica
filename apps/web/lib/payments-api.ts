const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export type PaymentsRealMoneyStatusSource =
  | "api-gateway"
  | "fallback:api-gateway-unavailable";

export interface PaymentsRealMoneyStatus {
  realMoneyEnabled: boolean;
  source: PaymentsRealMoneyStatusSource;
}

interface ApiPaymentsRealMoneyStatus {
  real_money_enabled: boolean;
}

export interface FetchPaymentsRealMoneyStatusOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
}

export async function fetchPaymentsRealMoneyStatus({
  baseUrl = process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  fetcher = fetch,
}: FetchPaymentsRealMoneyStatusOptions = {}): Promise<PaymentsRealMoneyStatus> {
  const url = new URL(
    "/v1/payments/real-money-status",
    trimTrailingSlash(baseUrl) + "/",
  );

  try {
    const response = await fetcher(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`api-gateway request failed: ${response.status}`);
    }

    const payload = (await response.json()) as ApiPaymentsRealMoneyStatus;
    return {
      realMoneyEnabled: payload.real_money_enabled === true,
      source: "api-gateway",
    };
  } catch {
    return {
      realMoneyEnabled: false,
      source: "fallback:api-gateway-unavailable",
    };
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
