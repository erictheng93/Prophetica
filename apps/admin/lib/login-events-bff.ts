import "server-only";

const DEFAULT_USER_SERVICE_URL = "http://127.0.0.1:8084";

export interface AdminLoginEvent {
  id: string;
  login_surface: string;
  principal_type: string;
  principal_id?: string | null;
  user_id?: string | null;
  principal_external_id?: string | null;
  event_type: string;
  occurred_at_epoch_seconds: number;
  ip_address: string;
  ip_source: string;
  geo_country?: string | null;
  geo_region?: string | null;
  geo_city?: string | null;
  geo_accuracy: string;
  user_agent_raw: string;
  browser_name?: string | null;
  device_type: string;
  auth_method: string;
  failure_reason?: string | null;
  request_id?: string | null;
  correlation_id?: string | null;
}

export async function fetchAdminLoginEvents(
  upstreamSessionToken: string,
  options: {
    limit?: number;
    fetcher?: typeof fetch;
    baseUrl?: string;
  } = {},
): Promise<AdminLoginEvent[]> {
  const baseUrl = trimTrailingSlash(
    options.baseUrl ?? process.env.USER_SERVICE_URL ?? DEFAULT_USER_SERVICE_URL,
  );
  const params = new URLSearchParams({
    limit: String(options.limit ?? 50),
  });
  const response = await (options.fetcher ?? fetch)(
    `${baseUrl}/v1/internal/admin/login-events?${params.toString()}`,
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

  const body = (await response.json()) as { events?: AdminLoginEvent[] };
  return Array.isArray(body.events) ? body.events : [];
}

export function adminLoginLocationLabel(event: AdminLoginEvent): string {
  return (
    event.geo_city?.trim() ||
    event.geo_region?.trim() ||
    event.geo_country?.trim() ||
    "未知"
  );
}

export function adminLoginDeviceLabel(event: AdminLoginEvent): string {
  return event.device_type?.trim() && event.device_type !== "unknown"
    ? event.device_type
    : "未知";
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
