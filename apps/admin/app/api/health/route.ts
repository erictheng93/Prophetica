// SPEC-017 section 6.3 - admin app liveness probe.

export function GET() {
  return Response.json({ status: "ok", service: "admin" });
}
