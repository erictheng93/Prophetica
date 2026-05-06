// SPEC-017 §6.3 — admin app liveness probe.

export const runtime = "edge";
export const dynamic = "force-static";

export function GET() {
  return Response.json({ status: "ok", service: "admin" });
}
