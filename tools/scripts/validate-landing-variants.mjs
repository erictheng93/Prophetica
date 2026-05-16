#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const KEBAB_CASE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const REGION_CODE = /^[A-Z]{2}$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isNonEmptyString(value) {
  return typeof value === "string" && value.length > 0;
}

function normalizePath(root, filePath) {
  return path.relative(root, filePath).split(path.sep).join("/");
}

async function findLandingPackages(root) {
  const appsDir = path.join(root, "apps");
  let entries;

  try {
    entries = await readdir(appsDir, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  return entries
    .filter((entry) => entry.isDirectory() && entry.name.startsWith("landing-"))
    .map((entry) => {
      const slug = entry.name.slice("landing-".length);
      return {
        slug,
        filePath: path.join(appsDir, entry.name, "package.json"),
      };
    })
    .sort((a, b) => a.filePath.localeCompare(b.filePath));
}

function validateStringField(errors, relPath, value, fieldName) {
  if (!isNonEmptyString(value)) {
    errors.push(
      `${relPath}: prophetica.variant.${fieldName} must be a non-empty string.`,
    );
  }
}

function validateDateField(errors, relPath, value, fieldName) {
  if (!isNonEmptyString(value)) {
    errors.push(
      `${relPath}: prophetica.variant.${fieldName} must be a non-empty string.`,
    );
    return;
  }

  if (!DATE.test(value)) {
    errors.push(
      `${relPath}: prophetica.variant.${fieldName} must be YYYY-MM-DD.`,
    );
  }
}

function validateUniqueStringArray(
  errors,
  relPath,
  value,
  fieldName,
  options = {},
) {
  if (!Array.isArray(value) || value.length === 0) {
    errors.push(
      `${relPath}: prophetica.variant.${fieldName} must be a non-empty array.`,
    );
    return;
  }

  const seen = new Set();
  const duplicateValues = new Set();

  value.forEach((item, index) => {
    if (!isNonEmptyString(item)) {
      errors.push(
        `${relPath}: prophetica.variant.${fieldName}[${index}] must be a non-empty string.`,
      );
      return;
    }

    if (options.pattern && !options.pattern.test(item)) {
      errors.push(
        `${relPath}: prophetica.variant.${fieldName}[${index}] must ${options.patternMessage}.`,
      );
    }

    if (seen.has(item)) {
      duplicateValues.add(item);
    }
    seen.add(item);
  });

  for (const duplicateValue of [...duplicateValues].sort()) {
    errors.push(
      `${relPath}: prophetica.variant.${fieldName} contains duplicate value "${duplicateValue}".`,
    );
  }
}

function validateVariant({ relPath, slug, packageJson }) {
  const errors = [];
  const expectedName = `@prophetica/landing-${slug}`;

  if (packageJson.name !== expectedName) {
    errors.push(`${relPath}: name must be "${expectedName}".`);
  }

  const variant = packageJson.prophetica?.variant;
  if (!isObject(variant)) {
    errors.push(`${relPath}: prophetica.variant must be an object.`);
    return { errors, variantId: undefined };
  }

  if (variant.id !== slug) {
    errors.push(
      `${relPath}: prophetica.variant.id must equal directory slug "${slug}".`,
    );
  }
  if (!isNonEmptyString(variant.id) || !KEBAB_CASE.test(variant.id)) {
    errors.push(`${relPath}: prophetica.variant.id must be kebab-case.`);
  }

  validateUniqueStringArray(errors, relPath, variant.regions, "regions", {
    pattern: REGION_CODE,
    patternMessage: "match /^[A-Z]{2}$/",
  });
  validateUniqueStringArray(
    errors,
    relPath,
    variant.product_lines,
    "product_lines",
    {
      pattern: KEBAB_CASE,
      patternMessage: "be kebab-case",
    },
  );
  validateStringField(errors, relPath, variant.design_theme, "design_theme");
  validateStringField(errors, relPath, variant.owner, "owner");
  validateDateField(errors, relPath, variant.launched_at, "launched_at");

  const deployment = variant.deployment;
  if (!isObject(deployment)) {
    errors.push(`${relPath}: prophetica.variant.deployment must be an object.`);
  } else {
    validateStringField(
      errors,
      relPath,
      deployment.domain,
      "deployment.domain",
    );
    if (deployment.path_prefix !== undefined) {
      if (
        typeof deployment.path_prefix !== "string" ||
        !deployment.path_prefix.startsWith("/")
      ) {
        errors.push(
          `${relPath}: prophetica.variant.deployment.path_prefix must start with "/".`,
        );
      }
    }
  }

  if (
    variant.retired_at !== null &&
    !(typeof variant.retired_at === "string" && DATE.test(variant.retired_at))
  ) {
    errors.push(
      `${relPath}: prophetica.variant.retired_at must be null or YYYY-MM-DD.`,
    );
  }

  return { errors, variantId: variant.id };
}

export async function validateLandingVariants(root = process.cwd()) {
  const resolvedRoot = path.resolve(root);
  const packages = await findLandingPackages(resolvedRoot);
  const errors = [];
  const idDeclarations = new Map();

  for (const landingPackage of packages) {
    const relPath = normalizePath(resolvedRoot, landingPackage.filePath);
    let packageJson;

    try {
      packageJson = JSON.parse(await readFile(landingPackage.filePath, "utf8"));
    } catch (error) {
      errors.push(
        `${relPath}: package.json must be valid JSON (${error.message}).`,
      );
      continue;
    }

    const result = validateVariant({
      relPath,
      slug: landingPackage.slug,
      packageJson,
    });
    errors.push(...result.errors);

    if (isNonEmptyString(result.variantId)) {
      const declarations = idDeclarations.get(result.variantId) ?? [];
      declarations.push(relPath);
      idDeclarations.set(result.variantId, declarations);
    }
  }

  for (const [variantId, declarations] of [...idDeclarations.entries()].sort(
    ([a], [b]) => a.localeCompare(b),
  )) {
    if (declarations.length > 1) {
      errors.push(
        `prophetica.variant.id "${variantId}" is declared by multiple landing packages: ${declarations
          .sort()
          .join(", ")}.`,
      );
    }
  }

  return { errors: errors.sort() };
}

async function main() {
  const root = process.argv[2] ?? process.cwd();
  const { errors } = await validateLandingVariants(root);

  if (errors.length === 0) {
    console.log("Landing variant manifests are valid.");
    return;
  }

  console.error("Landing variant manifest validation failed:");
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exitCode = 1;
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : "";
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}
