// SPEC-021 — landing-sports liveness probe.

export const dynamic = "force-static";

export function GET() {
  return Response.json({
    status: "ok",
    service: "landing-sports",
    variant: "sports",
  });
}
