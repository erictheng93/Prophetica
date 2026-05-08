import assert from "node:assert/strict";
import { test } from "node:test";

import { mapLocaleToSumsub } from "../lib/kyc-locale.js";

test("zh-Hant (next-intl script tag) maps to zh-TW for SumSub", () => {
  // The repo's only locale today is zh-Hant; SumSub does not recognise the
  // script subtag, so this single mapping is the entire point of the helper.
  assert.equal(mapLocaleToSumsub("zh-Hant"), "zh-TW");
});

test("region tags pass through unchanged when SumSub recognises them", () => {
  assert.equal(mapLocaleToSumsub("zh-TW"), "zh-TW");
  assert.equal(mapLocaleToSumsub("zh-CN"), "zh-CN");
  assert.equal(mapLocaleToSumsub("ja-JP"), "ja-JP");
  assert.equal(mapLocaleToSumsub("ko-KR"), "ko-KR");
  assert.equal(mapLocaleToSumsub("vi-VN"), "vi-VN");
  assert.equal(mapLocaleToSumsub("ms-MY"), "ms-MY");
});

test("language-only codes resolve to their canonical region per SPEC-005a §2", () => {
  assert.equal(mapLocaleToSumsub("ja"), "ja-JP");
  assert.equal(mapLocaleToSumsub("ko"), "ko-KR");
  assert.equal(mapLocaleToSumsub("vi"), "vi-VN");
  assert.equal(mapLocaleToSumsub("ms"), "ms-MY");
});

test("English variants normalise to bare 'en' (SumSub uses no en-US/en-GB split)", () => {
  assert.equal(mapLocaleToSumsub("en"), "en");
  assert.equal(mapLocaleToSumsub("en-US"), "en");
  assert.equal(mapLocaleToSumsub("en-GB"), "en");
});

test("simplified Chinese script tag maps to zh-CN", () => {
  assert.equal(mapLocaleToSumsub("zh-Hans"), "zh-CN");
});

test("unknown locales fall back to English rather than passing through", () => {
  // Passing an unknown code through would cause SumSub to silently ignore it
  // and fall back to its own default — better to default explicitly here.
  assert.equal(mapLocaleToSumsub("fr-FR"), "en");
  assert.equal(mapLocaleToSumsub(""), "en");
  assert.equal(mapLocaleToSumsub("xx-YY"), "en");
});
