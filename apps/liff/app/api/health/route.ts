// SPEC-017 §6.3 — liff app liveness probe.

export const dynamic = "force-static";

export function GET() {
  return Response.json({ status: "ok", service: "liff" });
}
