import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { validateLandingVariants } from "./validate-landing-variants.mjs";

async function makeRoot() {
  return await mkdtemp(path.join(os.tmpdir(), "prophetica-landing-variants-"));
}

async function writeVariant(root, slug, overrides = {}) {
  const appDir = path.join(root, "apps", `landing-${slug}`);
  await mkdir(appDir, { recursive: true });

  const packageJson = {
    name: `@prophetica/landing-${slug}`,
    prophetica: {
      variant: {
        id: slug,
        regions: ["TW"],
        product_lines: ["sports"],
        design_theme: "apple",
        deployment: {
          domain: `${slug}.prophetica.example`,
          path_prefix: "/",
        },
        owner: "marketing",
        launched_at: "2026-06-01",
        retired_at: null,
      },
    },
  };

  const merged = overrides.rawPackage ?? {
    ...packageJson,
    ...overrides,
    prophetica: {
      ...packageJson.prophetica,
      ...overrides.prophetica,
      variant: {
        ...packageJson.prophetica.variant,
        ...overrides.prophetica?.variant,
        deployment: {
          ...packageJson.prophetica.variant.deployment,
          ...overrides.prophetica?.variant?.deployment,
        },
      },
    },
  };

  await writeFile(
    path.join(appDir, "package.json"),
    `${JSON.stringify(merged, null, 2)}\n`,
  );
}

test("accepts valid landing variant package manifests", async () => {
  const root = await makeRoot();
  await writeVariant(root, "sports");
  await writeVariant(root, "jp-casino", {
    prophetica: {
      variant: {
        regions: ["JP"],
        product_lines: ["casino", "sports"],
        deployment: {
          domain: "jp.prophetica.example",
          path_prefix: "/casino",
        },
        retired_at: "2027-01-31",
      },
    },
  });

  const result = await validateLandingVariants(root);

  assert.deepEqual(result.errors, []);
});

test("reports deterministic schema, naming, and uniqueness errors", async () => {
  const root = await makeRoot();
  await writeVariant(root, "sports", {
    name: "@prophetica/wrong",
    prophetica: {
      variant: {
        regions: ["TW", "tw", "TW"],
        product_lines: ["sports", "", "sports", "Live Odds"],
        design_theme: "",
        deployment: {
          domain: "",
          path_prefix: "casino",
        },
        owner: "",
        launched_at: "",
        retired_at: "31-01-2027",
      },
    },
  });
  await writeVariant(root, "bad-id", {
    prophetica: {
      variant: {
        id: "bad!",
        regions: ["PH"],
      },
    },
  });
  await writeVariant(root, "casino", {
    prophetica: {
      variant: {
        id: "sports",
        regions: ["JP"],
      },
    },
  });

  const { errors } = await validateLandingVariants(root);

  assert.deepEqual(errors, [
    "apps/landing-bad-id/package.json: prophetica.variant.id must be kebab-case.",
    'apps/landing-bad-id/package.json: prophetica.variant.id must equal directory slug "bad-id".',
    'apps/landing-casino/package.json: prophetica.variant.id must equal directory slug "casino".',
    'apps/landing-sports/package.json: name must be "@prophetica/landing-sports".',
    "apps/landing-sports/package.json: prophetica.variant.deployment.domain must be a non-empty string.",
    'apps/landing-sports/package.json: prophetica.variant.deployment.path_prefix must start with "/".',
    "apps/landing-sports/package.json: prophetica.variant.design_theme must be a non-empty string.",
    "apps/landing-sports/package.json: prophetica.variant.launched_at must be a non-empty string.",
    "apps/landing-sports/package.json: prophetica.variant.owner must be a non-empty string.",
    'apps/landing-sports/package.json: prophetica.variant.product_lines contains duplicate value "sports".',
    "apps/landing-sports/package.json: prophetica.variant.product_lines[1] must be a non-empty string.",
    "apps/landing-sports/package.json: prophetica.variant.product_lines[3] must be kebab-case.",
    'apps/landing-sports/package.json: prophetica.variant.regions contains duplicate value "TW".',
    "apps/landing-sports/package.json: prophetica.variant.regions[1] must match /^[A-Z]{2}$/.",
    "apps/landing-sports/package.json: prophetica.variant.retired_at must be null or YYYY-MM-DD.",
    'prophetica.variant.id "sports" is declared by multiple landing packages: apps/landing-casino/package.json, apps/landing-sports/package.json.',
  ]);
});

test("requires a prophetica.variant object and validates empty arrays", async () => {
  const root = await makeRoot();
  await writeVariant(root, "empty", {
    prophetica: {
      variant: {
        regions: [],
        product_lines: [],
      },
    },
  });
  await writeVariant(root, "missing", {
    rawPackage: {
      name: "@prophetica/landing-missing",
      prophetica: {},
    },
  });

  const { errors } = await validateLandingVariants(root);

  assert.deepEqual(errors, [
    "apps/landing-empty/package.json: prophetica.variant.product_lines must be a non-empty array.",
    "apps/landing-empty/package.json: prophetica.variant.regions must be a non-empty array.",
    "apps/landing-missing/package.json: prophetica.variant must be an object.",
  ]);
});

test("requires launched_at to use YYYY-MM-DD format", async () => {
  const root = await makeRoot();
  await writeVariant(root, "sports", {
    prophetica: {
      variant: {
        launched_at: "06/01/2026",
      },
    },
  });

  const { errors } = await validateLandingVariants(root);

  assert.deepEqual(errors, [
    "apps/landing-sports/package.json: prophetica.variant.launched_at must be YYYY-MM-DD.",
  ]);
});
