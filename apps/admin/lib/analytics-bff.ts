import type { SessionPayload } from "./session-crypto.js";
import { signInternalCall } from "./internal-call.js";

type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export interface ProxyAnalyticsInput {
  viewer: SessionPayload | null;
  method: string;
  adminPath: string;
  body?: string;
  gatewayUrl?: string;
  secret?: string;
  nowSeconds?: number;
  fetchImpl?: FetchLike;
}

export async function proxyAnalyticsRequest(input: ProxyAnalyticsInput): Promise<Response> {
  if (!input.viewer || (input.viewer.role !== "admin" && input.viewer.role !== "manager")) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const gatewayUrl = input.gatewayUrl ?? process.env.API_GATEWAY_URL;
  if (!gatewayUrl) {
    return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
  }

  const body = input.body ?? "";
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: input.method,
    path: signaturePath,
    body,
    actorId: input.viewer.sub,
    role: input.viewer.role,
    secret: input.secret,
    nowSeconds: input.nowSeconds,
  });
  const requestHeaders: Record<string, string> = {
    ...headers,
    "x-request-id": crypto.randomUUID(),
  };
  if (body.length > 0) {
    requestHeaders["content-type"] = "application/json";
  }

  const upstreamUrl = new URL(input.adminPath, gatewayUrl).toString();
  const upstream = await (input.fetchImpl ?? fetch)(upstreamUrl, {
    method: input.method,
    headers: requestHeaders,
    body: body.length > 0 ? body : undefined,
    cache: "no-store",
  });

  const responseHeaders = new Headers();
  for (const name of ["content-type", "content-disposition"]) {
    const value = upstream.headers.get(name);
    if (value) responseHeaders.set(name, value);
  }

  return new Response(await upstream.arrayBuffer(), {
    status: upstream.status,
    headers: responseHeaders,
  });
}
