import assert from "node:assert/strict";
import { test } from "node:test";

import {
  FAQ_SUPPORTED_LOCALES,
  FaqCatalogValidationError,
  InMemoryFaqCatalogStore,
  KvFaqCatalogStore,
  backfillFaqCatalogMissingLocalizedText,
  createDefaultFaqCatalog,
  faqCatalogToPublicSections,
  normalizeFaqCatalog,
  parseStoredFaqCatalog,
  serializeFaqCatalog,
} from "../src/support-faq/index.js";

test("default FAQ catalog exposes the SPEC-012 support categories", () => {
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "system:test",
  });

  assert.equal(catalog.schemaVersion, 1);
  assert.deepEqual(
    catalog.sections.map((section) => section.id),
    ["account", "topup", "kyc", "settlement", "rg", "security"],
  );
  assert.equal(catalog.sections[0]?.items[0]?.id, "forgotPassword");
});

test("default FAQ catalog includes copy for every supported locale and enabled entry", () => {
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "system:test",
  });

  for (const locale of FAQ_SUPPORTED_LOCALES) {
    for (const section of catalog.sections) {
      if (!section.enabled) continue;
      assert.ok(
        section.title[locale]?.trim(),
        `${section.id} needs ${locale} title`,
      );

      for (const item of section.items) {
        if (!item.enabled) continue;
        assert.ok(
          item.question[locale]?.trim(),
          `${section.id}.${item.id} needs ${locale} question`,
        );
        assert.ok(
          item.answer[locale]?.trim(),
          `${section.id}.${item.id} needs ${locale} answer`,
        );
      }
    }
  }

  const sections = faqCatalogToPublicSections(catalog, "zh-Hans");
  assert.equal(sections[0]?.title, "账号与登录");
  assert.equal(sections[0]?.items[0]?.question, "忘记密码怎么办？");

  assert.equal(
    faqCatalogToPublicSections(catalog, "id")[0]?.title,
    "Akun dan masuk",
  );
  assert.equal(
    faqCatalogToPublicSections(catalog, "vi")[0]?.title,
    "Tài khoản và đăng nhập",
  );
  assert.equal(
    faqCatalogToPublicSections(catalog, "tl")[0]?.title,
    "Account at pag-sign in",
  );
});

test("public FAQ sections are ordered, enabled-only, and locale-fallback aware", () => {
  const catalog = normalizeFaqCatalog({
    schemaVersion: 1,
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "admin-1",
    sections: [
      {
        id: "billing",
        order: 20,
        enabled: true,
        title: { "zh-Hant": "帳務", en: "Billing" },
        items: [
          {
            id: "disabled",
            order: 0,
            enabled: false,
            question: { "zh-Hant": "不顯示" },
            answer: { "zh-Hant": "不顯示" },
          },
          {
            id: "missing-en",
            order: 1,
            enabled: true,
            question: { "zh-Hant": "點數何時入帳？" },
            answer: { "zh-Hant": "通常五分鐘內。" },
          },
        ],
      },
      {
        id: "account",
        order: 10,
        enabled: true,
        title: { "zh-Hant": "帳號", en: "Account" },
        items: [
          {
            id: "reset",
            order: 1,
            enabled: true,
            question: {
              "zh-Hant": "如何重設密碼？",
              en: "How do I reset my password?",
            },
            answer: {
              "zh-Hant": "使用登入頁的忘記密碼。",
              en: "Use the forgot-password link.",
            },
          },
        ],
      },
    ],
  });

  const sections = faqCatalogToPublicSections(catalog, "en");

  assert.deepEqual(
    sections.map((section) => section.id),
    ["account", "billing"],
  );
  assert.equal(sections[0]?.title, "Account");
  assert.equal(sections[0]?.items[0]?.question, "How do I reset my password?");
  assert.equal(sections[1]?.items.length, 1);
  assert.equal(sections[1]?.items[0]?.question, "點數何時入帳？");
});

test("FAQ catalog backfill restores missing default localized copy without overwriting edits", () => {
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "admin-1",
  });
  const section = catalog.sections[0]!;
  const item = section.items[0]!;

  delete section.title.id;
  item.question.id = "Pertanyaan kustom";
  delete item.answer.id;

  const backfilled = backfillFaqCatalogMissingLocalizedText(catalog);
  const backfilledSection = backfilled.sections[0]!;
  const backfilledItem = backfilledSection.items[0]!;

  assert.equal(backfilled.updatedAt, "2026-05-13T00:00:00.000Z");
  assert.equal(backfilledSection.title.id, "Akun dan masuk");
  assert.equal(backfilledItem.question.id, "Pertanyaan kustom");
  assert.equal(
    backfilledItem.answer.id,
    "Gunakan alur lupa kata sandi di halaman masuk. Kami akan mengirim tautan reset ke alamat email yang sudah diverifikasi.",
  );
});

test("FAQ catalog normalization rejects duplicate ids and empty enabled copy", () => {
  assert.throws(
    () =>
      normalizeFaqCatalog({
        schemaVersion: 1,
        updatedAt: "2026-05-13T00:00:00.000Z",
        updatedBy: "admin-1",
        sections: [
          {
            id: "account",
            order: 1,
            enabled: true,
            title: { "zh-Hant": "帳號" },
            items: [
              {
                id: "same",
                order: 1,
                enabled: true,
                question: { "zh-Hant": "有效問題？" },
                answer: { "zh-Hant": "" },
              },
              {
                id: "same",
                order: 2,
                enabled: true,
                question: { "zh-Hant": "另一題？" },
                answer: { "zh-Hant": "回答" },
              },
            ],
          },
        ],
      }),
    FaqCatalogValidationError,
  );
});

test("stored FAQ catalog round-trips through JSON with validation", () => {
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "system:test",
  });

  const serialized = serializeFaqCatalog(catalog);
  const parsed = parseStoredFaqCatalog(serialized);

  assert.deepEqual(parsed, catalog);
  assert.throws(() => parseStoredFaqCatalog("{"), FaqCatalogValidationError);
});

test("in-memory FAQ store returns clones and supports reset", async () => {
  const store = new InMemoryFaqCatalogStore();
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "system:test",
  });

  assert.equal(await store.load(), null);
  await store.save(catalog);

  const loaded = await store.load();
  assert.deepEqual(loaded, catalog);
  loaded?.sections.push({
    id: "mutated",
    order: 99,
    enabled: true,
    title: { "zh-Hant": "不應污染 store" },
    items: [],
  });

  assert.equal(
    (await store.load())?.sections.some((section) => section.id === "mutated"),
    false,
  );

  await store.reset();
  assert.equal(await store.load(), null);
});

test("KV FAQ store serializes with one stable key and validates loaded JSON", async () => {
  const writes = new Map<string, string>();
  const kv = {
    async get(key: string): Promise<string | null> {
      return writes.get(key) ?? null;
    },
    async put(key: string, value: string): Promise<void> {
      writes.set(key, value);
    },
    async delete(key: string): Promise<void> {
      writes.delete(key);
    },
  };
  const store = new KvFaqCatalogStore(kv, { key: "support:faq:test" });
  const catalog = createDefaultFaqCatalog({
    updatedAt: "2026-05-13T00:00:00.000Z",
    updatedBy: "system:test",
  });

  await store.save(catalog);

  assert.equal(writes.has("support:faq:test"), true);
  assert.deepEqual(await store.load(), catalog);

  writes.set("support:faq:test", "{}");
  await assert.rejects(() => store.load(), FaqCatalogValidationError);

  await store.reset();
  assert.equal(writes.has("support:faq:test"), false);
});
