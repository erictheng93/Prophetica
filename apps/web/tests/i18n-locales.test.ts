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

test("web i18n supports the launch locales with Traditional Chinese as default", () => {
  assert.equal(DEFAULT_LOCALE, "zh-Hant");
  assert.deepEqual(SUPPORTED_LOCALES, ["zh-Hant", "zh-Hans", "en", "id", "vi", "tl"]);
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
  assert.equal(resolveAcceptLanguageLocale("fr-FR, en;q=0.8, zh-CN;q=0.9"), "zh-Hans");
  assert.equal(resolveAcceptLanguageLocale("fr-FR, fil-PH;q=0.7, id-ID;q=0.9"), "id");
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

test("non-default catalogues cover the public top-nav page namespaces", () => {
  const defaultMessages = readMessages(DEFAULT_LOCALE);
  const requiredNamespaces = [
    "markets",
    "howItWorks",
    "responsibleGaming.metadata",
    "responsibleGaming.overview",
    "support.overview",
  ];

  for (const locale of SUPPORTED_LOCALES) {
    if (locale === DEFAULT_LOCALE) continue;

    const messages = readMessages(locale);

    for (const namespace of requiredNamespaces) {
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
