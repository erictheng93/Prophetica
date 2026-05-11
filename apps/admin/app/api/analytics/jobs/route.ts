import type { NextRequest } from "next/server";
import { proxyToGateway } from "../proxy";

export async function GET(request: NextRequest) {
  const query = request.nextUrl.search;
  return proxyToGateway("GET", `/api/internal/admin/report-jobs${query}`);
}

export async function POST(request: NextRequest) {
  return proxyToGateway("POST", "/api/internal/admin/report-jobs", await request.text());
}
