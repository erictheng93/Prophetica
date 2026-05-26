import assert from "node:assert/strict";
import { test } from "node:test";

import { mapLocaleToProvider } from "../lib/provider-locale.js";

test("zh-Hant (next-intl script tag) maps to zh-TW for iProov", () => {
  // The repo's only locale today is zh-Hant; iProov does not recognise the
  // script subtag, so this single mapping is the entire point of the helper.
  assert.equal(mapLocaleToProvider("zh-Hant"), "zh-TW");
});

test("region tags pass through unchanged when iProov recognises them", () => {
  assert.equal(mapLocaleToProvider("zh-TW"), "zh-TW");
  assert.equal(mapLocaleToProvider("zh-CN"), "zh-CN");
  assert.equal(mapLocaleToProvider("ja-JP"), "ja-JP");
  assert.equal(mapLocaleToProvider("ko-KR"), "ko-KR");
  assert.equal(mapLocaleToProvider("vi-VN"), "vi-VN");
  assert.equal(mapLocaleToProvider("ms-MY"), "ms-MY");
});

test("language-only codes resolve to their canonical region per SPEC-005a §2", () => {
  assert.equal(mapLocaleToProvider("ja"), "ja-JP");
  assert.equal(mapLocaleToProvider("ko"), "ko-KR");
  assert.equal(mapLocaleToProvider("vi"), "vi-VN");
  assert.equal(mapLocaleToProvider("ms"), "ms-MY");
});

test("English variants normalise to bare 'en' (iProov uses no en-US/en-GB split)", () => {
  assert.equal(mapLocaleToProvider("en"), "en");
  assert.equal(mapLocaleToProvider("en-US"), "en");
  assert.equal(mapLocaleToProvider("en-GB"), "en");
});

test("supported app locales without iProov support fall back to English", () => {
  assert.equal(mapLocaleToProvider("id"), "en");
  assert.equal(mapLocaleToProvider("id-ID"), "en");
  assert.equal(mapLocaleToProvider("tl"), "en");
  assert.equal(mapLocaleToProvider("fil"), "en");
});

test("simplified Chinese script tag maps to zh-CN", () => {
  assert.equal(mapLocaleToProvider("zh-Hans"), "zh-CN");
});

test("unknown locales fall back to English rather than passing through", () => {
  // Passing an unknown code through would cause iProov to silently ignore it
  // and fall back to its own default — better to default explicitly here.
  assert.equal(mapLocaleToProvider("fr-FR"), "en");
  assert.equal(mapLocaleToProvider(""), "en");
  assert.equal(mapLocaleToProvider("xx-YY"), "en");
});
