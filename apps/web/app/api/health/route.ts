// SPEC-017 §6.2 — Next.js app must expose /api/health for liveness probing.

export const dynamic = "force-static";

export function GET() {
  return Response.json({ status: "ok", service: "web" });
}
