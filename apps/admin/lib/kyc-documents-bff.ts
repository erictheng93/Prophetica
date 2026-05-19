import "server-only";
import { signInternalCall } from "./internal-call.js";
import type { SessionPayload } from "./session-crypto.js";

interface ProxyInput {
  userId: string;
  adminPath: string;
}
interface ProxyDeps {
  gatewayUrl?: string;
  secret?: string;
  fetchImpl?: typeof fetch;
}

const IMAGE_CONTENT_TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);

export async function proxyKycDocumentRequest(
  viewer: SessionPayload,
  input: ProxyInput,
  deps: ProxyDeps = {},
): Promise<Response> {
  const gatewayUrl = deps.gatewayUrl ?? process.env.API_GATEWAY_URL;
  if (!gatewayUrl) {
    return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
  }
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: "GET",
    path: signaturePath,
    body: "",
    actorId: viewer.sub,
    role: viewer.role,
    secret: deps.secret,
  });
  const doFetch = deps.fetchImpl ?? fetch;
  let upstream: Response;
  try {
    upstream = await doFetch(new URL(input.adminPath, gatewayUrl), {
      method: "GET",
      headers: { ...headers, "x-request-id": crypto.randomUUID() },
      cache: "no-store",
    });
  } catch {
    return Response.json({ error: "gateway_unavailable" }, { status: 502 });
  }

  const contentType = upstream.headers.get("content-type") ?? "application/octet-stream";
  const responseHeaders = new Headers();
  responseHeaders.set("content-type", contentType);
  responseHeaders.set("cache-control", "private, no-store");
  responseHeaders.set("pragma", "no-cache");
  responseHeaders.set("x-content-type-options", "nosniff");

  if (contentType.startsWith("image/") && !IMAGE_CONTENT_TYPES.has(contentType)) {
    return Response.json({ error: "unsupported_media_type" }, { status: 502 });
  }
  return new Response(upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  });
}
