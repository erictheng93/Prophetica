import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  type SupportedLocale,
  resolveAcceptLanguageLocale,
  resolveSupportedLocale,
} from "../i18n/locales.js";

type MessageTree = Record<string, unknown>;

const CHINESE_LOCALES: SupportedLocale[] = ["zh-Hant", "zh-Hans"];
const NON_CHINESE_LOCALES = SUPPORTED_LOCALES.filter(
  (locale) => !CHINESE_LOCALES.includes(locale),
);

const localizedNamespaces = [
  "common",
  "sectionNav",
  "support.overview",
  "markets",
  "howItWorks",
  "responsibleGaming.metadata",
  "responsibleGaming.overview",
  "responsibleGaming.selfExclusion",
  "stubPage",
  "landing",
  "offline",
  "kyc.privacyNotice",
];

const deferredTranslationNamespaces = [
  "about",
  "accountAffiliate",
  "affiliate",
  "auth.signIn",
  "connectionStatus",
  "kyc.layout",
  "kyc.overview",
  "kyc.status",
  "kyc.tier1",
  "kyc.tier2",
  "kyc.tier3",
  "kyc.tierStepper",
  "legal.stubs",
  "marketCard",
  "marketDetail",
  "portfolio",
  "probabilityChart",
  "responsibleGaming.cooldown",
  "responsibleGaming.helpline",
  "responsibleGaming.limits",
  "responsibleGaming.resources",
  "settlement",
  "support.contact",
  "support.disputes",
  "support.faq",
  "topup",
  "tradePanel",
];

const maxDeferredTranslationLeafCount: Partial<
  Record<SupportedLocale, number>
> = {
  "zh-Hans": 866,
  en: 866,
  id: 866,
  vi: 866,
  tl: 866,
};

function readMessages(locale: SupportedLocale): MessageTree {
  return JSON.parse(
    readFileSync(join(process.cwd(), `messages/${locale}.json`), "utf8"),
  ) as MessageTree;
}

function hasPath(messages: MessageTree, path: string): boolean {
  let value: unknown = messages;

  for (const segment of path.split(".")) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return false;
    }

    value = (value as MessageTree)[segment];
  }

  return value !== undefined;
}

function valueAtPath(messages: MessageTree, path: string): unknown {
  let value: unknown = messages;

  for (const segment of path.split(".")) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return undefined;
    }

    value = (value as MessageTree)[segment];
  }

  return value;
}

function leafPaths(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }

  return Object.entries(value as MessageTree).flatMap(([key, child]) =>
    leafPaths(child, prefix ? `${prefix}.${key}` : key),
  );
}

function stringLeafEntries(value: unknown, prefix = ""): [string, string][] {
  if (typeof value === "string") {
    return prefix ? [[prefix, value]] : [];
  }

  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [];
  }

  return Object.entries(value as MessageTree).flatMap(([key, child]) =>
    stringLeafEntries(child, prefix ? `${prefix}.${key}` : key),
  );
}

function pathIsWithinNamespace(path: string, namespace: string): boolean {
  return path === namespace || path.startsWith(`${namespace}.`);
}

test("web i18n supports the launch locales with Traditional Chinese as default", () => {
  assert.equal(DEFAULT_LOCALE, "zh-Hant");
  assert.deepEqual(SUPPORTED_LOCALES, [
    "zh-Hant",
    "zh-Hans",
    "en",
    "id",
    "vi",
    "tl",
  ]);
});

test("locale resolver normalises browser and legacy language tags", () => {
  assert.equal(resolveSupportedLocale("zh-TW"), "zh-Hant");
  assert.equal(resolveSupportedLocale("zh-CN"), "zh-Hans");
  assert.equal(resolveSupportedLocale("en-US"), "en");
  assert.equal(resolveSupportedLocale("id-ID"), "id");
  assert.equal(resolveSupportedLocale("vi-VN"), "vi");
  assert.equal(resolveSupportedLocale("fil-PH"), "tl");
  assert.equal(resolveSupportedLocale("fr-FR"), undefined);
});

test("accept-language resolver respects q-weighted supported candidates", () => {
  assert.equal(
    resolveAcceptLanguageLocale("fr-FR, en;q=0.8, zh-CN;q=0.9"),
    "zh-Hans",
  );
  assert.equal(
    resolveAcceptLanguageLocale("fr-FR, fil-PH;q=0.7, id-ID;q=0.9"),
    "id",
  );
  assert.equal(resolveAcceptLanguageLocale("fr-FR"), undefined);
});

test("every supported locale has a parseable message catalogue", () => {
  for (const locale of SUPPORTED_LOCALES) {
    const messages = readMessages(locale);

    assert.ok(messages.common, `${locale} missing common namespace`);
    assert.ok(messages.landing, `${locale} missing landing namespace`);
    assert.ok(messages.offline, `${locale} missing offline namespace`);
  }
});

test("non-default catalogues cover localized namespaces before runtime fallback", () => {
  const defaultMessages = readMessages(DEFAULT_LOCALE);

  for (const locale of SUPPORTED_LOCALES) {
    if (locale === DEFAULT_LOCALE) continue;

    const messages = readMessages(locale);

    for (const namespace of localizedNamespaces) {
      assert.ok(hasPath(messages, namespace), `${locale} missing ${namespace}`);
      const defaultNamespace = valueAtPath(defaultMessages, namespace);
      const localeNamespace = valueAtPath(messages, namespace);

      for (const key of leafPaths(defaultNamespace)) {
        assert.ok(
          hasPath(localeNamespace as MessageTree, key),
          `${locale} missing ${namespace}.${key}`,
        );
      }
    }
  }
});

test("remaining non-default fallback coverage is explicit and cannot grow silently", () => {
  const defaultMessages = readMessages(DEFAULT_LOCALE);
  const defaultLeafPaths = leafPaths(defaultMessages);

  for (const locale of SUPPORTED_LOCALES) {
    if (locale === DEFAULT_LOCALE) continue;

    const messages = readMessages(locale);
    const missingPaths = defaultLeafPaths.filter(
      (path) => !hasPath(messages, path),
    );
    const untrackedPaths = missingPaths.filter(
      (path) =>
        !deferredTranslationNamespaces.some((namespace) =>
          pathIsWithinNamespace(path, namespace),
        ),
    );

    assert.deepEqual(
      untrackedPaths,
      [],
      `${locale} has untracked zh-Hant fallback keys`,
    );

    assert.ok(
      missingPaths.length <= (maxDeferredTranslationLeafCount[locale] ?? 0),
      `${locale} fallback debt grew from ${
        maxDeferredTranslationLeafCount[locale]
      } to ${missingPaths.length} keys`,
    );
  }
});

test("non-Chinese catalogues do not contain Chinese copy", () => {
  const cjk = /[\u3400-\u9fff]/u;

  for (const locale of NON_CHINESE_LOCALES) {
    const messages = readMessages(locale);

    for (const [path, value] of stringLeafEntries(messages)) {
      assert.doesNotMatch(value, cjk, `${locale}.${path}`);
    }
  }
});

test("Vietnamese sensitive compliance copy keeps tone marks", () => {
  const vietnameseTone =
    /[ăâđêôơưĂÂĐÊÔƠƯáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ]/u;
  const technicalValuesWithoutToneMarks = new Set([
    "kyc.privacyNotice.matrix.rows.legalName.collected",
    "kyc.privacyNotice.matrix.rows.birthday.collected",
  ]);
  const sensitiveNamespaces = [
    "responsibleGaming.selfExclusion",
    "kyc.privacyNotice",
  ];
  const messages = readMessages("vi");

  for (const namespace of sensitiveNamespaces) {
    for (const [path, value] of stringLeafEntries(
      valueAtPath(messages, namespace),
    )) {
      const fullPath = `${namespace}.${path}`;

      if (
        value.length > 12 &&
        /[a-z]{3,}/i.test(value) &&
        !technicalValuesWithoutToneMarks.has(fullPath)
      ) {
        assert.match(
          value,
          vietnameseTone,
          `vi.${fullPath} appears to be diacritic-stripped`,
        );
      }
    }
  }
});

test("how it works pricing copy uses market weighting instead of cent notation", () => {
  const messages = JSON.parse(
    readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
  ) as {
    howItWorks: {
      metadata: Record<string, string>;
      pricing: Record<string, unknown>;
    };
  };

  const publicCopy = JSON.stringify({
    metadata: messages.howItWorks.metadata,
    pricing: messages.howItWorks.pricing,
  });

  assert.doesNotMatch(publicCopy, /\bcent\b|¢/i);
  assert.match(publicCopy, /市場權重/);
  assert.match(publicCopy, /YES 權重/);
});
