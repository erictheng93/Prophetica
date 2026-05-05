// SPEC-017 §6.2 — Next.js app must expose /api/health for liveness probing.
// Kept on the default Node runtime; if Cloudflare Pages adapter requires Edge,
// add `export const runtime = 'edge';` (ADR-0004 §10 V-2 / V-4 verification).

export const dynamic = "force-static";

export function GET() {
  return Response.json({ status: "ok", service: "web" });
}
