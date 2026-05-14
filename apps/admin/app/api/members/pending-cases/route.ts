import { getViewer } from "@/lib/session";
import { proxyMemberPendingCasesRequest } from "@/lib/member-management-bff";

export async function GET(request: Request) {
  return proxyMemberPendingCasesRequest({
    viewer: await getViewer(),
    searchParams: new URL(request.url).searchParams,
  });
}
