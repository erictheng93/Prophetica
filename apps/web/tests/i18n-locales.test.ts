import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  resolveAcceptLanguageLocale,
  resolveSupportedLocale,
} from "../i18n/locales.js";

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
    const messages = JSON.parse(
      readFileSync(join(process.cwd(), `messages/${locale}.json`), "utf8"),
    ) as Record<string, unknown>;

    assert.ok(messages.common, `${locale} missing common namespace`);
    assert.ok(messages.landing, `${locale} missing landing namespace`);
    assert.ok(messages.offline, `${locale} missing offline namespace`);
  }
});
