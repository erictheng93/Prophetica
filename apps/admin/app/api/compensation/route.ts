import type { NextRequest } from "next/server";
import {
  createCompensationProposal,
  listCompensationProposals,
} from "@/lib/compensation-bff";
import type {
  CompensationProposalStatus,
  CompensationProposerKind,
} from "@/lib/compensation";
import { getViewer } from "@/lib/session";
import type { SessionPayload } from "@/lib/session-crypto";

export async function GET(request: NextRequest) {
  const viewer = await getViewer();
  if (!canUseCompensationWorkspace(viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  return listCompensationProposals(viewer, {
    status: parseStatus(request.nextUrl.searchParams.get("status")),
    proposerKind: parseProposerKind(
      request.nextUrl.searchParams.get("proposer_kind") ??
        request.nextUrl.searchParams.get("proposerKind"),
    ),
    page: parsePositiveInt(request.nextUrl.searchParams.get("page")),
    pageSize: parsePositiveInt(request.nextUrl.searchParams.get("page_size")),
  });
}

export async function POST(request: NextRequest) {
  const viewer = await getViewer();
  if (!canUseCompensationWorkspace(viewer)) {
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
  const userId =
    typeof candidate.user_id === "string"
      ? candidate.user_id.trim()
      : typeof candidate.userId === "string"
        ? candidate.userId.trim()
        : "";
  if (!userId) {
    return Response.json({ error: "invalid_user_id" }, { status: 400 });
  }

  return createCompensationProposal(viewer, {
    userId,
    ticket_id:
      typeof candidate.ticket_id === "string"
        ? candidate.ticket_id
        : undefined,
    reason: typeof candidate.reason === "string" ? candidate.reason : undefined,
    amount_minor_units:
      typeof candidate.amount_minor_units === "number"
        ? candidate.amount_minor_units
        : undefined,
  });
}

function canUseCompensationWorkspace(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager";
}

function parsePositiveInt(value: string | null): number | undefined {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

function parseStatus(value: string | null): CompensationProposalStatus | undefined {
  return value === "proposed" ||
    value === "approved" ||
    value === "posting" ||
    value === "posted" ||
    value === "posting_failed" ||
    value === "posting_unknown" ||
    value === "rejected" ||
    value === "cancelled"
    ? value
    : undefined;
}

function parseProposerKind(
  value: string | null,
): CompensationProposerKind | undefined {
  return value === "cs_system" ||
    value === "manager" ||
    value === "admin" ||
    value === "affiliate"
    ? value
    : undefined;
}
