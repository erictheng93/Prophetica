import assert from "node:assert/strict";
import { test } from "node:test";

import {
  InMemoryFaqCatalogStore,
  createDefaultFaqCatalog,
} from "@prophetica/ts-sdk/support-faq";
import { loadManagedFaqSections } from "../lib/support-faq.js";

test("managed FAQ loader returns null when no catalog is published", async () => {
  const sections = await loadManagedFaqSections({
    locale: "zh-Hant",
    store: new InMemoryFaqCatalogStore(),
  });

  assert.equal(sections, null);
});

test("managed FAQ loader converts a published catalog into public sections", async () => {
  const store = new InMemoryFaqCatalogStore();
  await store.save(
    createDefaultFaqCatalog({
      updatedAt: "2026-05-13T00:00:00.000Z",
      updatedBy: "admin-1",
    }),
  );

  const sections = await loadManagedFaqSections({
    locale: "en",
    store,
  });

  assert.equal(sections?.[0]?.id, "account");
  assert.equal(sections?.[0]?.title, "Account and sign-in");
  assert.equal(sections?.[0]?.items[0]?.id, "forgotPassword");
  assert.equal(sections?.[0]?.items[0]?.question, "How do I reset my password?");
});
