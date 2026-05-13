import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { test } from "node:test";

const sourceRoot = process.cwd();
const nextConfig = readFileSync(join(sourceRoot, "next.config.mjs"), "utf8");
const sourceFilePattern = /\.(?:ts|tsx)$/;
const localJsImportPattern =
  /from\s+["'](\.[^"']+\.js)["']|import\(["'](\.[^"']+\.js)["']\)/g;
const ignoredPathSegments = new Set([
  ".next",
  ".test-out",
  "node_modules",
  "tests",
]);

test("Turbopack can resolve NodeNext-style local .js imports used by web source", () => {
  const unresolvedImports = sourceFiles(sourceRoot).flatMap((filePath) => {
    const source = readFileSync(filePath, "utf8");
    const imports = [...source.matchAll(localJsImportPattern)].map(
      (match) => match[1] ?? match[2] ?? "",
    );

    return imports
      .filter((specifier) => !existsSync(resolve(dirname(filePath), specifier)))
      .filter((specifier) => !hasTurbopackAlias(specifier))
      .map(
        (specifier) => `${relative(sourceRoot, filePath)} imports ${specifier}`,
      );
  });

  assert.deepEqual(unresolvedImports, []);
});

function sourceFiles(root: string): string[] {
  return readdirRecursive(root).filter((filePath) => {
    const relativePath = relative(root, filePath);
    const segments = relativePath.split(/[\\/]/);
    return (
      sourceFilePattern.test(filePath) &&
      segments.every((segment) => !ignoredPathSegments.has(segment))
    );
  });
}

function readdirRecursive(root: string): string[] {
  const entries = readdirSync(root, {
    withFileTypes: true,
  });

  return entries.flatMap((entry) => {
    const path = join(root, entry.name);
    return entry.isDirectory() ? readdirRecursive(path) : [path];
  });
}

function hasTurbopackAlias(specifier: string): boolean {
  const escapedSpecifier = specifier.replaceAll(".", "\\.");
  return new RegExp(`["']${escapedSpecifier}["']\\s*:`).test(nextConfig);
}
