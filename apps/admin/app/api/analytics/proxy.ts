import { getViewer } from "@/lib/session";
import { proxyAnalyticsRequest } from "@/lib/analytics-bff";

export async function proxyToGateway(method: string, adminPath: string, body = ""): Promise<Response> {
  return proxyAnalyticsRequest({
    viewer: await getViewer(),
    method,
    adminPath,
    body,
  });
}
