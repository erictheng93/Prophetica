import { NextResponse } from "next/server";
import {
  loadFaqCatalogForViewer,
  resetFaqCatalogForViewer,
  saveFaqCatalogForViewer,
} from "@/lib/faq-admin";
import { getFaqCatalogStore } from "@/lib/faq-store";
import { getViewer } from "@/lib/session";

export async function GET() {
  const viewer = await getViewer();
  if (!viewer) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const body = await loadFaqCatalogForViewer({
    viewer,
    store: getFaqCatalogStore(),
  });
  return NextResponse.json(body);
}

export async function PUT(request: Request) {
  const viewer = await getViewer();
  if (!viewer) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const payload = (await request.json()) as { catalog?: unknown };
  const response = await saveFaqCatalogForViewer({
    viewer,
    store: getFaqCatalogStore(),
    catalog: payload.catalog,
  });

  return NextResponse.json(response.body, { status: response.status });
}

export async function DELETE() {
  const viewer = await getViewer();
  if (!viewer) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const response = await resetFaqCatalogForViewer({
    viewer,
    store: getFaqCatalogStore(),
  });

  return NextResponse.json(response.body, { status: response.status });
}
