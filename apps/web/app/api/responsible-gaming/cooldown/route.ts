import {
  COOLDOWN_DURATIONS,
  ResponsibleGamingApiError,
  fetchCooldownStatus,
  startCooldown,
  type CooldownDuration,
} from "@/lib/responsible-gaming-api";
import { getUpstreamSessionToken } from "@/lib/session";

export const dynamic = "force-dynamic";

export async function GET() {
  const bearerToken = (await getUpstreamSessionToken()) ?? undefined;
  return Response.json(await fetchCooldownStatus({ bearerToken }));
}

export async function POST(request: Request) {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return Response.json(
      { error: "invalid_json", message: "Request body must be valid JSON" },
      { status: 400 },
    );
  }

  if (!isStartRequest(body)) {
    return Response.json(
      {
        error: "invalid_cooldown_request",
        message: "Cooldown request requires duration and consent",
      },
      { status: 400 },
    );
  }

  try {
    const bearerToken = (await getUpstreamSessionToken()) ?? undefined;
    return Response.json(await startCooldown(body, { bearerToken }));
  } catch (error) {
    if (error instanceof ResponsibleGamingApiError) {
      return Response.json(
        { error: error.code, message: error.message },
        { status: error.status },
      );
    }

    return Response.json(
      {
        error: "responsible_gaming_unavailable",
        message: "Responsible gaming service is unavailable",
      },
      { status: 502 },
    );
  }
}

function isStartRequest(
  value: unknown,
): value is { duration: CooldownDuration; consent: boolean } {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return (
    isCooldownDuration(record.duration) && typeof record.consent === "boolean"
  );
}

function isCooldownDuration(value: unknown): value is CooldownDuration {
  return COOLDOWN_DURATIONS.some((duration) => duration === value);
}
