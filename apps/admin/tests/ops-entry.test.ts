import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

const readAdminFile = (path: string) =>
  readFileSync(join(process.cwd(), path), "utf8");

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

test("admin affiliate operations are grouped under the management area", () => {
  const facadeSource = readAdminFile("lib/ops-facade.ts");
  const shellSource = readAdminFile("components/admin-shell.tsx");
  const sidebarSource = readAdminFile("components/admin-nav.tsx");
  const drawerSource = readAdminFile("components/admin-nav-drawer.tsx");

  assert.match(facadeSource, /代理管理區/);
  assert.match(facadeSource, /代理清單/);
  assert.match(facadeSource, /佣金管理/);
  assert.match(facadeSource, /撥款管理/);
  assert.match(facadeSource, /\/affiliate\/commissions/);
  assert.match(facadeSource, /\/affiliate\/payouts/);
  assert.match(shellSource, /children:\s*route\.children/);
  assert.match(sidebarSource, /aria-label=\{`\$\{link\.label\} 子導覽`\}/);
  assert.match(drawerSource, /aria-label=\{`\$\{link\.label\} 子導覽`\}/);
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

  assert.match(
    packageJson.scripts?.test ?? "",
    /tsc --project tsconfig\.test\.json/,
  );
  assert.match(
    packageJson.scripts?.test ?? "",
    /node .*--test \.test-out\/tests\/\*\.test\.js/,
  );
});

test("admin worker has the shared FAQ content KV binding", () => {
  const wranglerConfig = readAdminFile("wrangler.toml");

  assert.match(wranglerConfig, /^\[\[kv_namespaces\]\]$/m);
  assert.match(wranglerConfig, /^binding = ["']FAQ_CONTENT_KV["']$/m);
});

test("admin dev server allows configured LAN origins", async () => {
  const nextConfigUrl = pathToFileURL(join(process.cwd(), "next.config.mjs"));
  nextConfigUrl.search = `test=${Date.now()}`;
  const nextConfig = (await import(nextConfigUrl.href)) as {
    default: { allowedDevOrigins?: string[] };
    parseAllowedDevOrigins: (
      value?: string,
      localOrigins?: string[],
    ) => string[];
  };

  assert.ok(nextConfig.default.allowedDevOrigins?.includes("192.168.68.93"));
  assert.ok(nextConfig.default.allowedDevOrigins?.some((origin) => origin.length > 0));
  assert.deepEqual(
    nextConfig.parseAllowedDevOrigins("http://192.168.68.94:3002", []),
    ["192.168.68.93", "192.168.68.94"],
  );
  assert.deepEqual(nextConfig.parseAllowedDevOrigins(undefined, ["172.20.10.6"]), [
    "192.168.68.93",
    "172.20.10.6",
  ]);
});

test("decision support dashboard stays read-only and covers required insight areas", () => {
  const dashboardSource = readAdminFile("app/dashboard/page.tsx");
  const copySource = readAdminFile("lib/admin-copy.ts");

  assert.match(dashboardSource, /@\/lib\/analytics-reporting/);
  assert.match(dashboardSource, /AnalyticsUnavailableState/);
  assert.match(dashboardSource, /decision-support/);
  assert.match(dashboardSource, /kpi/);
  assert.match(dashboardSource, /alerts/);
  assert.match(dashboardSource, /versioned-reports/);
  assert.match(dashboardSource, /audit-trail/);
  assert.match(dashboardSource, /availableFormats/);
  assert.match(dashboardSource, /retention/);
  assert.match(dashboardSource, /rowHash/);
  assert.match(copySource, /unavailableTitle/);
  assert.doesNotMatch(dashboardSource, /<form\b/);
});

test("analytics templates page has an unavailable state for disconnected reporting services", () => {
  const analyticsSource = readAdminFile("app/analytics/page.tsx");
  const copySource = readAdminFile("lib/admin-copy.ts");

  assert.match(analyticsSource, /AnalyticsUnavailableState/);
  assert.match(analyticsSource, /templatesUnavailableTitle/);
  assert.match(copySource, /templatesUnavailableTitle/);
});

test("server-side analytics fetch forwards the production internal mTLS header", () => {
  const serverFetchSource = readAdminFile("lib/analytics-server-fetch.ts");

  assert.match(serverFetchSource, /x-internal-mtls/);
});

test("admin copy defaults to Traditional Chinese with an i18n-ready copy module", () => {
  const copySource = readAdminFile("lib/admin-copy.ts");
  const overviewSource = readAdminFile("app/page.tsx");
  const dashboardSource = readAdminFile("app/dashboard/page.tsx");
  const shellSource = readAdminFile("components/admin-shell.tsx");

  assert.match(copySource, /adminLocale\s*=\s*"zh-Hant"/);
  assert.match(copySource, /adminCopy/);
  assert.match(copySource, /管理操作入口/);
  assert.match(copySource, /決策支援儀表板/);
  assert.match(overviewSource, /adminCopy\.overview/);
  assert.match(dashboardSource, /adminCopy\.dashboard/);
  assert.match(shellSource, /adminCopy\.shell/);
});
