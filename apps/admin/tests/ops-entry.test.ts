import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const readAdminFile = (path: string) => readFileSync(join(process.cwd(), path), "utf8");

const operationalDomains = [
  "member-management",
  "points-exceptions",
  "affiliate-review",
  "kyc-risk-events",
  "compensation-reviews",
  "manual-approvals",
] as const;

test("admin operations entry page exposes all six operational domains", () => {
  const source = readAdminFile("app/page.tsx");

  for (const domain of operationalDomains) {
    assert.match(source, new RegExp(domain), domain);
  }
});

test("admin operations entry page reads from the server-side ops facade", () => {
  const source = readAdminFile("app/page.tsx");

  assert.match(source, /@\/lib\/ops-facade/);
  assert.match(source, /\bgetAdminOperationsSnapshot\b/);
});

test("admin pages do not ship forms with unsupported aria-disabled", () => {
  const pageFiles = [
    "app/page.tsx",
    "app/members/page.tsx",
    "app/points/page.tsx",
    "app/affiliate/page.tsx",
    "app/kyc-risk/page.tsx",
    "app/compensation/page.tsx",
    "app/approvals/page.tsx",
  ];

  for (const file of pageFiles) {
    const source = readAdminFile(file);

    assert.doesNotMatch(source, /<form\b[^>]*\baria-disabled=/, file);
  }
});

test("admin test script typechecks then runs compiled node tests", () => {
  const packageJson = JSON.parse(readAdminFile("package.json")) as {
    scripts?: Record<string, string>;
  };

  assert.match(packageJson.scripts?.test ?? "", /tsc --project tsconfig\.test\.json/);
  assert.match(packageJson.scripts?.test ?? "", /node --test \.test-out\/tests\/\*\.test\.js/);
});

test("decision support dashboard stays read-only and covers required insight areas", () => {
  const dashboardSource = readAdminFile("app/dashboard/page.tsx");

  assert.match(dashboardSource, /decision-support/);
  assert.match(dashboardSource, /kpi/);
  assert.match(dashboardSource, /alerts/);
  assert.match(dashboardSource, /versioned-reports/);
  assert.match(dashboardSource, /audit-trail/);
  assert.doesNotMatch(dashboardSource, /<form\b/);
});
