import { proxyToGateway } from "../../proxy";

export async function GET() {
  return proxyToGateway("GET", "/api/internal/admin/report-jobs/latest-per-template");
}
