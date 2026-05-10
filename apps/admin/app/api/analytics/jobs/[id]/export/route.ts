import { NextResponse } from "next/server";
import {
  exportReportJob,
  getAnalyticsReportingSnapshot,
  type ReportFormat,
} from "@/lib/analytics-reporting";
import { getViewer } from "@/lib/session";

const ALLOWED_FORMATS: ReportFormat[] = ["csv", "json", "xlsx"];

function isReportFormat(value: string): value is ReportFormat {
  return (ALLOWED_FORMATS as readonly string[]).includes(value);
}

interface RouteContext {
  params: Promise<{ id: string }>;
}

/*
 * SPEC-020 §3 export endpoint. Phase 2 regenerates the deterministic
 * snapshot per request and looks up the job by id, so the body is stable
 * for the same admin/manager session. Phase 3 will swap this for a
 * proxy to analytics-service that streams the real file from S3 / R2.
 */
export async function GET(request: Request, context: RouteContext) {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }

  const { id } = await context.params;
  const url = new URL(request.url);
  const format = url.searchParams.get("format") ?? "csv";
  if (!isReportFormat(format)) {
    return NextResponse.json({ error: "invalid_format" }, { status: 400 });
  }

  const snapshot = await getAnalyticsReportingSnapshot(viewer.role);
  const job = snapshot.reports.find((row) => row.id === id);
  if (!job) {
    return NextResponse.json({ error: "job_not_found" }, { status: 404 });
  }

  const payload = exportReportJob(job, format);
  // Detach the underlying ArrayBuffer slice so the typing matches
  // NextResponse's BodyInit. The library returns a Uint8Array backed by
  // ArrayBufferLike (a TS 5.x narrowing) that the dom lib refuses to
  // accept directly; the slice copy is cheap and tightens the type.
  const slice = payload.body.slice().buffer;
  return new NextResponse(slice, {
    status: 200,
    headers: {
      "content-type": payload.contentType,
      "content-disposition": `attachment; filename="${payload.filename}"`,
      "cache-control": "no-store",
    },
  });
}
