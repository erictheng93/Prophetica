const DEFAULT_USER_SERVICE_URL = "http://127.0.0.1:8084";

export interface SelfLoginEvent {
  id: string;
  occurred_at_epoch_seconds: number;
  event_type: string;
  login_surface: string;
  ip_masked: string;
  ip_source: string;
  location_label: string;
  device_label: string;
  browser_name?: string | null;
  auth_method: string;
}

export interface SelfLoginEventsResponse {
  events: SelfLoginEvent[];
}

export async function fetchSelfLoginEvents(
  upstreamSessionToken: string,
  options: {
    limit?: number;
    fetcher?: typeof fetch;
    baseUrl?: string;
  } = {},
): Promise<SelfLoginEvent[]> {
  const baseUrl = trimTrailingSlash(
    options.baseUrl ?? process.env.USER_SERVICE_URL ?? DEFAULT_USER_SERVICE_URL,
  );
  const params = new URLSearchParams({
    limit: String(options.limit ?? 20),
  });
  const response = await (options.fetcher ?? fetch)(
    `${baseUrl}/v1/users/me/login-events?${params.toString()}`,
    {
      cache: "no-store",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${upstreamSessionToken}`,
      },
    },
  );

  if (!response.ok) {
    return [];
  }

  const body = (await response.json()) as Partial<SelfLoginEventsResponse>;
  return Array.isArray(body.events) ? body.events : [];
}

export function loginEventLocationLabel(event: SelfLoginEvent): string {
  return event.location_label?.trim() || "未知";
}

export function loginEventDeviceLabel(event: SelfLoginEvent): string {
  return event.device_label?.trim() || "未知";
}

export function loginEventBrowserLabel(event: SelfLoginEvent): string {
  return event.browser_name?.trim() || "未知";
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
