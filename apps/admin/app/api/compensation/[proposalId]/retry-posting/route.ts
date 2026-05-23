import { retryPostingForProposal } from "@/lib/compensation-bff";
import { getViewer } from "@/lib/session";
import type { SessionPayload } from "@/lib/session-crypto";

export async function POST(
  _request: Request,
  { params }: { params: Promise<{ proposalId: string }> },
) {
  const viewer = await getViewer();
  if (!canRetryCompensationPosting(viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const { proposalId } = await params;
  return retryPostingForProposal(viewer, proposalId);
}

function canRetryCompensationPosting(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin";
}
