import assert from "node:assert/strict";
import { test } from "node:test";

import {
  InMemoryFaqCatalogStore,
  createDefaultFaqCatalog,
} from "@prophetica/ts-sdk/support-faq";
import {
  loadFaqCatalogForViewer,
  resetFaqCatalogForViewer,
  saveFaqCatalogForViewer,
} from "../lib/faq-admin.js";

const adminViewer = {
  sub: "U-ADMIN-001",
  role: "admin" as const,
  iat: 1,
  exp: 2,
};

const managerViewer = {
  sub: "U-MANAGER-001",
  role: "manager" as const,
  iat: 1,
  exp: 2,
};

test("admin FAQ loader falls back to the default catalog when no managed catalog exists", async () => {
  const response = await loadFaqCatalogForViewer({
    viewer: adminViewer,
    store: new InMemoryFaqCatalogStore(),
    now: () => "2026-05-13T00:00:00.000Z",
  });

  assert.equal(response.source, "default");
  assert.equal(response.canEdit, true);
  assert.equal(response.catalog.updatedBy, "system:default-faq");
  assert.deepEqual(
    response.catalog.sections.map((section) => section.id),
    ["account", "topup", "kyc", "settlement", "rg", "security"],
  );
});

test("admin FAQ saver stamps actor metadata and persists a normalized catalog", async () => {
  const store = new InMemoryFaqCatalogStore();
  const draft = createDefaultFaqCatalog({
    updatedAt: "2026-05-12T00:00:00.000Z",
    updatedBy: "system:test",
  });
  draft.sections.reverse();

  const response = await saveFaqCatalogForViewer({
    viewer: adminViewer,
    store,
    catalog: draft,
    now: () => "2026-05-13T12:34:56.000Z",
  });

  assert.equal(response.status, 200);
  assert.equal(response.body.source, "managed");
  assert.equal(response.body.catalog.updatedBy, "U-ADMIN-001");
  assert.equal(response.body.catalog.updatedAt, "2026-05-13T12:34:56.000Z");
  assert.deepEqual(
    response.body.catalog.sections.map((section) => section.id),
    ["account", "topup", "kyc", "settlement", "rg", "security"],
  );
  assert.deepEqual(await store.load(), response.body.catalog);
});

test("manager cannot publish or reset FAQ catalog", async () => {
  const store = new InMemoryFaqCatalogStore();
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-12T00:00:00.000Z",
    updatedBy: "system:test",
  });

  const saveResponse = await saveFaqCatalogForViewer({
    viewer: managerViewer,
    store,
    catalog,
    now: () => "2026-05-13T12:34:56.000Z",
  });
  const resetResponse = await resetFaqCatalogForViewer({
    viewer: managerViewer,
    store,
  });

  assert.equal(saveResponse.status, 403);
  assert.equal(resetResponse.status, 403);
  assert.equal(await store.load(), null);
});

test("admin can reset managed FAQ content back to defaults", async () => {
  const store = new InMemoryFaqCatalogStore();
  await store.save(
    createDefaultFaqCatalog({
      updatedAt: "2026-05-12T00:00:00.000Z",
      updatedBy: "system:test",
    }),
  );

  const response = await resetFaqCatalogForViewer({
    viewer: adminViewer,
    store,
  });

  assert.equal(response.status, 200);
  assert.equal(response.body.source, "default");
  assert.equal(await store.load(), null);
});
