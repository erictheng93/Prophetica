import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const readSportsFile = (path) =>
  readFileSync(join(process.cwd(), path), "utf8");

test("sports landing uses the Black Gold cinematic direction", () => {
  const source = readSportsFile("app/page.tsx");
  const messages = JSON.parse(readSportsFile("messages/zh-Hant.json"));

  assert.match(source, /blackGoldLuxuryDesign/);
  assert.match(source, /BlackGoldTileMedia/);
  assert.equal(messages.landingSports.hero.headline, "洞察戰局，量化眼光");
  assert.match(source, /bg-\[var\(--sports-bg\)\]/);
  assert.match(source, /#D4A857/);
});

test("sports landing follows the main site ProphTile page structure", () => {
  const source = readSportsFile("app/page.tsx");

  assert.match(source, /ProphTile/);
  assert.equal(source.match(/<ProphTile/g)?.length, 5);
  assert.match(source, /surface="light"/);
  assert.match(source, /surface="dark"/);
  assert.match(source, /surface="parchment"/);
  assert.match(source, /surface="dark-2"/);
  assert.match(source, /<BlackGoldTileMedia/);
});

test("sports landing keeps signup tracking and compliance copy", () => {
  const source = readSportsFile("app/page.tsx");

  assert.match(source, /TrackedSignupButton/);
  assert.match(source, /placement="hero"/);
  assert.match(source, /placement="start"/);
  assert.match(source, /neutral\.headline/);
  assert.match(source, /responsibleGaming\.headline/);
});

test("sports landing wires next-intl through config and root layout", () => {
  const nextConfig = readSportsFile("next.config.mjs");
  const layout = readSportsFile("app/layout.tsx");
  const requestConfig = readSportsFile("i18n/request.ts");

  assert.match(nextConfig, /next-intl\/plugin/);
  assert.match(layout, /NextIntlClientProvider/);
  assert.match(layout, /getLocale/);
  assert.match(layout, /getMessages/);
  assert.match(requestConfig, /getRequestConfig/);
  assert.match(requestConfig, /LOCALE_COOKIE_NAME/);
});

test("sports landing exposes localized nav, auth actions, and theme toggle", () => {
  const nav = readSportsFile("lib/site-nav.tsx");
  const page = readSportsFile("app/page.tsx");
  const tailwindConfig = readSportsFile("tailwind.config.ts");

  assert.match(nav, /LanguageSwitcher/);
  assert.match(nav, /SiteThemeToggle/);
  assert.match(nav, /href="\/auth\/sign-in"/);
  assert.match(nav, /href="\/auth\/sign-in\?mode=signup"/);
  assert.match(nav, /common\.auth/);
  assert.match(page, /getTranslations\("landingSports"\)/);
  assert.match(tailwindConfig, /"\.\/lib\/\*\*\/\*\.\{ts,tsx\}"/);
});

test("sports landing has parseable message catalogs for every supported locale", () => {
  const localesSource = readSportsFile("i18n/locales.ts");
  const locales = [
    ...localesSource.matchAll(/"([a-z]{2}(?:-[A-Za-z]+)?)"/g),
  ].map((match) => match[1]);
  const uniqueLocales = [...new Set(locales)].filter((locale) =>
    readdirSync(join(process.cwd(), "messages")).includes(`${locale}.json`),
  );

  assert.ok(uniqueLocales.length >= 3);

  for (const locale of uniqueLocales) {
    const messages = JSON.parse(readSportsFile(`messages/${locale}.json`));
    assert.ok(messages.common, `${locale} missing common namespace`);
    assert.ok(
      messages.landingSports,
      `${locale} missing landingSports namespace`,
    );
    assert.ok(messages.landingSports.hero, `${locale} missing hero copy`);
    assert.ok(messages.common.theme, `${locale} missing theme labels`);
  }
});

test("sports landing locale catalogs fully translate the landing page", () => {
  const defaultMessages = JSON.parse(readSportsFile("messages/zh-Hant.json"));
  const defaultLandingPaths = leafPaths(defaultMessages.landingSports);
  const defaultCommonPaths = leafPaths(defaultMessages.common);

  for (const locale of ["zh-Hans", "en", "id", "vi", "tl"]) {
    const messages = JSON.parse(readSportsFile(`messages/${locale}.json`));

    for (const path of defaultLandingPaths) {
      assert.ok(
        hasPath(messages.landingSports, path),
        `${locale} missing landingSports.${path}`,
      );
    }

    for (const path of defaultCommonPaths) {
      assert.ok(
        hasPath(messages.common, path),
        `${locale} missing common.${path}`,
      );
    }
  }
});

test("sports landing locale catalogs preserve value shapes for every locale", () => {
  const defaultMessages = JSON.parse(readSportsFile("messages/zh-Hant.json"));
  const defaultShapePaths = leafPaths(defaultMessages);

  for (const locale of ["zh-Hans", "en", "id", "vi", "tl"]) {
    const messages = JSON.parse(readSportsFile(`messages/${locale}.json`));

    for (const path of defaultShapePaths) {
      const defaultValue = getPath(defaultMessages, path);
      const localeValue = getPath(messages, path);

      assert.equal(
        Array.isArray(localeValue),
        Array.isArray(defaultValue),
        `${locale} ${path} must ${Array.isArray(defaultValue) ? "" : "not "}be an array`,
      );

      if (Array.isArray(defaultValue)) {
        assert.equal(
          localeValue.length,
          defaultValue.length,
          `${locale} ${path} must have ${defaultValue.length} entries`,
        );
      } else {
        assert.equal(
          typeof localeValue,
          "string",
          `${locale} ${path} must be text`,
        );
        assert.ok(localeValue.trim(), `${locale} ${path} must not be blank`);
      }
    }
  }
});

test("sports landing locale aliases cover browser language variants", () => {
  const localesSource = readSportsFile("i18n/locales.ts");

  assert.match(localesSource, /"zh-tw":\s*"zh-Hant"/);
  assert.match(localesSource, /"zh-cn":\s*"zh-Hans"/);
  assert.match(localesSource, /"id-id":\s*"id"/);
  assert.match(localesSource, /"vi-vn":\s*"vi"/);
  assert.match(localesSource, /fil:\s*"tl"/);
  assert.match(localesSource, /"fil-ph":\s*"tl"/);
});

test("sports landing catalogs do not contain mojibake replacement text", () => {
  for (const locale of ["zh-Hant", "zh-Hans", "en", "id", "vi", "tl"]) {
    const source = readSportsFile(`messages/${locale}.json`);
    assert.doesNotMatch(
      source,
      /[�廕廙羸繭穫璽籀]/u,
      `${locale} has mojibake`,
    );
  }
});

function leafPaths(value, prefix = "") {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }

  return Object.entries(value).flatMap(([key, child]) =>
    leafPaths(child, prefix ? `${prefix}.${key}` : key),
  );
}

function hasPath(value, path) {
  let current = value;
  for (const segment of path.split(".")) {
    if (!current || typeof current !== "object" || !(segment in current)) {
      return false;
    }
    current = current[segment];
  }
  return true;
}

function getPath(value, path) {
  let current = value;
  for (const segment of path.split(".")) {
    current = current[segment];
  }
  return current;
}
