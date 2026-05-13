import {
  backfillFaqCatalogMissingLocalizedText,
  createDefaultFaqCatalog,
  normalizeFaqCatalog,
  type FaqCatalog,
  type FaqCatalogStore,
} from "@prophetica/ts-sdk/support-faq";
import type { SessionPayload } from "./session-crypto.js";

export type FaqCatalogSource = "managed" | "default";

export interface FaqCatalogAdminView {
  catalog: FaqCatalog;
  source: FaqCatalogSource;
  canEdit: boolean;
}

export interface FaqAdminSuccess {
  status: 200;
  body: FaqCatalogAdminView;
}

export interface FaqAdminError {
  status: 400 | 403;
  body: {
    error: string;
    message: string;
  };
}

export type FaqAdminResponse = FaqAdminSuccess | FaqAdminError;

interface LoadFaqCatalogForViewerInput {
  viewer: SessionPayload;
  store: FaqCatalogStore;
  now?: () => string;
}

interface SaveFaqCatalogForViewerInput extends LoadFaqCatalogForViewerInput {
  catalog: unknown;
}

export async function loadFaqCatalogForViewer({
  viewer,
  store,
  now = () => new Date().toISOString(),
}: LoadFaqCatalogForViewerInput): Promise<FaqCatalogAdminView> {
  const managed = await store.load();
  const catalog = managed
    ? backfillFaqCatalogMissingLocalizedText(managed)
    : createDefaultFaqCatalog({
        updatedAt: now(),
        updatedBy: "system:default-faq",
      });

  return {
    catalog,
    source: managed ? "managed" : "default",
    canEdit: viewer.role === "admin",
  };
}

export async function saveFaqCatalogForViewer({
  viewer,
  store,
  catalog,
  now = () => new Date().toISOString(),
}: SaveFaqCatalogForViewerInput): Promise<FaqAdminResponse> {
  if (viewer.role !== "admin") {
    return forbidden();
  }

  let normalized: FaqCatalog;
  try {
    const draft = normalizeFaqCatalog(catalog);
    normalized = backfillFaqCatalogMissingLocalizedText({
      ...draft,
      updatedAt: now(),
      updatedBy: viewer.sub,
    });
  } catch (error) {
    return {
      status: 400,
      body: {
        error: "invalid_faq_catalog",
        message:
          error instanceof Error ? error.message : "FAQ catalog is invalid",
      },
    };
  }

  await store.save(normalized);
  return {
    status: 200,
    body: {
      catalog: normalized,
      source: "managed",
      canEdit: true,
    },
  };
}

export async function resetFaqCatalogForViewer({
  viewer,
  store,
  now = () => new Date().toISOString(),
}: LoadFaqCatalogForViewerInput): Promise<FaqAdminResponse> {
  if (viewer.role !== "admin") {
    return forbidden();
  }

  await store.reset();
  return {
    status: 200,
    body: await loadFaqCatalogForViewer({ viewer, store, now }),
  };
}

function forbidden(): FaqAdminError {
  return {
    status: 403,
    body: {
      error: "forbidden",
      message: "Only admin operators can publish FAQ changes.",
    },
  };
}
