import type { NextRequest } from "next/server";
import { reviewCompensationProposal } from "@/lib/compensation-bff";
import type { CompensationReviewDecision } from "@/lib/compensation";
import { getViewer } from "@/lib/session";
import type { SessionPayload } from "@/lib/session-crypto";

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ proposalId: string }> },
) {
  const viewer = await getViewer();
  if (!canReviewCompensation(viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }
  if (!payload || typeof payload !== "object") {
    return Response.json({ error: "invalid_request" }, { status: 400 });
  }
  const candidate = payload as Record<string, unknown>;
  const decision = parseDecision(candidate.decision);
  if (!decision) {
    return Response.json({ error: "invalid_decision" }, { status: 400 });
  }

  const { proposalId } = await params;
  return reviewCompensationProposal(
    viewer,
    proposalId,
    decision,
    typeof candidate.note === "string" ? candidate.note : undefined,
  );
}

function canReviewCompensation(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager";
}

function parseDecision(value: unknown): CompensationReviewDecision | null {
  return value === "approved" || value === "rejected" ? value : null;
}
