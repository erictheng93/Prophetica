import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

const parsedPort = Number.parseInt(process.env.ADMIN_PORT ?? "3002", 10);
const adminPort =
  Number.isNaN(parsedPort) || parsedPort <= 0 ? 3002 : parsedPort;

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

interface CrossDomainStreamEventForTest {
  correlationId: string;
  sourceDomain: (typeof operationalDomains)[number];
  targetDomains: (typeof operationalDomains)[number][];
  acknowledgement: {
    status: "confirmed";
    receiptId: string;
    returnedFrom: (typeof operationalDomains)[number];
    returnPath: string;
  };
}

test("admin operations entry page exposes all six operational domains", () => {
  const source = readAdminFile("app/page.tsx");

  for (const domain of operationalDomains) {
    assert.match(source, new RegExp(domain), domain);
  }
});

test("admin operations entry page reads live queue data from the server-side ops facade", () => {
  const source = readAdminFile("app/page.tsx");

  assert.match(source, /@\/lib\/ops-facade/);
  assert.match(source, /\bgetAdminOperationsLiveSnapshot\b/);
  assert.doesNotMatch(source, /\bgetAdminOperationsSnapshot\b/);
});

test("admin queue surfaces do not render the mock operations snapshot", () => {
  const layoutSource = readAdminFile("app/layout.tsx");
  const domainScopeSource = readAdminFile("components/domain-scope.tsx");

  assert.match(layoutSource, /\bgetAdminOperationsRoutes\b/);
  assert.doesNotMatch(layoutSource, /\bgetAdminOperationsSnapshot\b/);
  assert.match(domainScopeSource, /\bgetAdminOperationsLiveSnapshot\b/);
  assert.doesNotMatch(domainScopeSource, /\bgetAdminOperationsSnapshot\b/);
  assert.doesNotMatch(domainScopeSource, /snapshot\.queue\.slice/);
});

test("admin shell no longer owns persistent navigation chrome", () => {
  const shellSource = readAdminFile("components/admin-shell.tsx");

  assert.doesNotMatch(shellSource, /\bAdminSidebar\b/);
  assert.doesNotMatch(shellSource, /\bAdminNavDrawer\b/);
  assert.doesNotMatch(shellSource, /\bgetAdminOperationsRoutes\b/);
});

test("admin operations snapshot exposes confirmed cross-domain stream handoffs", async () => {
  const { getAdminOperationsSnapshot } =
    (await import("../lib/ops-facade.js")) as typeof import("../lib/ops-facade.js");
  const snapshot = getAdminOperationsSnapshot() as ReturnType<
    typeof getAdminOperationsSnapshot
  > & {
    crossDomainStream?: CrossDomainStreamEventForTest[];
  };
  const validDomains = new Set(operationalDomains);

  assert.ok(Array.isArray(snapshot.crossDomainStream));
  assert.ok(snapshot.crossDomainStream.length >= 3);
  assert.ok(
    snapshot.crossDomainStream.some((event) => event.targetDomains.length > 1),
    "at least one event should fan out to multiple operational domains",
  );

  const linkedDomains = new Set<string>();
  for (const event of snapshot.crossDomainStream) {
    assert.match(event.correlationId, /^corr-/);
    assert.ok(validDomains.has(event.sourceDomain), event.sourceDomain);
    linkedDomains.add(event.sourceDomain);

    for (const target of event.targetDomains) {
      assert.ok(validDomains.has(target), target);
      linkedDomains.add(target);
    }

    assert.equal(event.acknowledgement.status, "confirmed");
    assert.ok(
      event.acknowledgement.receiptId.startsWith("ack-"),
      event.acknowledgement.receiptId,
    );
    assert.ok(
      event.acknowledgement.returnedFrom.length > 0,
      event.acknowledgement.returnedFrom,
    );
    assert.ok(
      validDomains.has(event.acknowledgement.returnedFrom),
      event.acknowledgement.returnedFrom,
    );
    assert.match(event.acknowledgement.returnPath, / -> /);
  }

  assert.deepEqual([...linkedDomains].sort(), [...validDomains].sort());
});

test("admin operations snapshot derives every domain open item count from queue data", async () => {
  const { getAdminOperationsSnapshot } =
    (await import("../lib/ops-facade.js")) as typeof import("../lib/ops-facade.js");
  const snapshot = getAdminOperationsSnapshot();
  const expectedOpenItems = Object.fromEntries(
    operationalDomains.map((domain) => [domain, 0]),
  ) as Record<(typeof operationalDomains)[number], number>;

  for (const item of snapshot.queue) {
    expectedOpenItems[item.domain] += 1;
  }

  for (const route of snapshot.routes) {
    assert.equal(
      route.openItems,
      expectedOpenItems[route.domain],
      route.domain,
    );
  }

  const totalSummary = snapshot.summaries.find(
    (summary) => summary.label === "未結項目",
  );
  assert.equal(totalSummary?.value, String(snapshot.queue.length));
  assert.equal(
    snapshot.routes.find((route) => route.domain === "member-management")
      ?.openItems,
    0,
  );
});

test("admin operations live snapshot includes all member pending cases in domain counts", async () => {
  const previousApiGatewayUrl = process.env.API_GATEWAY_URL;
  const previousMemberFallback =
    process.env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK;
  const { getAdminOperationsLiveSnapshot } =
    (await import("../lib/ops-facade.js")) as typeof import("../lib/ops-facade.js");

  delete process.env.API_GATEWAY_URL;
  process.env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK = "true";

  try {
    const snapshot = await getAdminOperationsLiveSnapshot({
      sub: "U-MGR-001",
      role: "manager",
      iat: 1,
      exp: 2,
    });
    const queueIds = snapshot.queue.map((item) => item.id);

    assert.ok(queueIds.includes("case-MEM-1042"));
    assert.ok(queueIds.includes("case-user_123"));
    assert.equal(
      snapshot.routes.find((route) => route.domain === "kyc-risk-events")
        ?.openItems,
      2,
    );
    assert.equal(
      snapshot.routes.find((route) => route.domain === "member-management")
        ?.openItems,
      0,
    );
  } finally {
    if (previousApiGatewayUrl === undefined) {
      delete process.env.API_GATEWAY_URL;
    } else {
      process.env.API_GATEWAY_URL = previousApiGatewayUrl;
    }
    if (previousMemberFallback === undefined) {
      delete process.env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK;
    } else {
      process.env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK =
        previousMemberFallback;
    }
  }
});

test("overview marks cross-domain stream linkage as complete", () => {
  const pageSource = readAdminFile("app/page.tsx");
  const progressSource = readAdminFile(
    "components/implementation-progress.tsx",
  );

  assert.match(pageSource, /crossDomainStream=\{snapshot\.crossDomainStream\}/);
  assert.match(progressSource, /完成跨域串流/);
  assert.doesNotMatch(progressSource, /待補充跨域串流/);
});

test("overview marks notification reminder chain as complete", () => {
  const progressSource = readAdminFile(
    "components/implementation-progress.tsx",
  );

  assert.match(progressSource, /完成通知鏈路/);
  assert.match(progressSource, /notification-service API/);
  assert.match(progressSource, /Postgres outbox/);
  assert.doesNotMatch(progressSource, /待補充通知鏈路/);
});

test("members page exposes complete member management operations", () => {
  const source = readAdminFile("app/members/page.tsx");
  const kycReviewSource = readAdminFile("app/members/kyc-review/page.tsx");
  const approvalsSource = readAdminFile("app/approvals/page.tsx");
  const componentSource = readAdminFile("components/member-management.tsx");
  const bffSource = readAdminFile("lib/member-management-bff.ts");
  const modelSource = readAdminFile("lib/member-management.ts");
  const storeSource = readAdminFile("lib/member-management-store.ts");
  const rbacSource = readAdminFile("lib/rbac.ts");

  assert.match(source, /MemberManagement/);
  assert.doesNotMatch(source, /DomainScope/);
  assert.doesNotMatch(source, /showPendingCases/);
  assert.doesNotMatch(source, /showActionQueue/);
  assert.match(kycReviewSource, /showPendingCases/);
  assert.match(approvalsSource, /showActionQueue/);
  assert.match(approvalsSource, /showMemberWorkspace=\{false\}/);
  assert.match(componentSource, /會員查詢/);
  assert.ok(
    componentSource.indexOf("會員查詢") < componentSource.indexOf("會員名單"),
  );
  assert.match(componentSource, /role="dialog"/);
  assert.match(componentSource, /setMemberEditorOpen\(true\)/);
  assert.match(componentSource, /聯絡資料/);
  assert.match(componentSource, /帳戶摘要/);
  assert.match(componentSource, /狀態與風控/);
  assert.match(componentSource, /KYC 證件材料/);
  assert.match(componentSource, /KYC_DOCUMENTS_BUCKET/);
  assert.match(componentSource, /KYC_LIVENESS_BUCKET/);
  assert.match(componentSource, /提交處理動作/);
  assert.match(componentSource, /\/api\/members/);
  assert.match(bffSource, /proxyMemberSummaryRequest/);
  assert.match(bffSource, /proxyMemberActionRequest/);
  assert.match(modelSource, /listMemberSummaries/);
  assert.match(storeSource, /MEM-1042/);
  assert.match(storeSource, /sourceForAffiliate/);
  assert.match(rbacSource, /\/api\/members/);
});

test("admin affiliate operations are grouped under the management area", () => {
  const facadeSource = readAdminFile("lib/ops-facade.ts");
  const layoutSource = readAdminFile("app/layout.tsx");
  const sidebarSource = readAdminFile("components/admin-nav.tsx");
  const drawerSource = readAdminFile("components/admin-nav-drawer.tsx");

  assert.match(facadeSource, /label: "代理管理"/);
  assert.match(facadeSource, /代理清單/);
  assert.match(facadeSource, /佣金管理/);
  assert.match(facadeSource, /撥款管理/);
  assert.match(facadeSource, /\/affiliate\/commissions/);
  assert.match(facadeSource, /\/affiliate\/payouts/);
  assert.match(facadeSource, /KYC 審核/);
  assert.match(facadeSource, /\/members\/kyc-review/);
  assert.match(facadeSource, /工單審批/);
  assert.match(facadeSource, /\/approvals/);
  assert.match(facadeSource, /navHidden: true/);
  assert.match(
    layoutSource,
    /const filteredChildren = route\.children\s*\?\.filter/,
  );
  assert.match(layoutSource, /route\.navHidden/);
  assert.match(layoutSource, /\.\.\.\(filteredChildren/);
  assert.match(layoutSource, /inDevelopmentHrefs/);
  assert.match(facadeSource, /inDevelopment: true/);
  assert.match(sidebarSource, /aria-label=\{`\$\{link\.label\} 子導覽`\}/);
  assert.match(drawerSource, /aria-label=\{`\$\{link\.label\} 子導覽`\}/);
});

test("affiliate portal dashboard reads are render-pass cached by stable primitives", () => {
  const bffSource = readAdminFile("lib/affiliate-management-bff.ts");

  assert.match(bffSource, /import \{ cache \} from "react"/);
  assert.match(bffSource, /const fetchAffiliateDashboardForViewer = cache/);
  assert.match(
    bffSource,
    /return fetchAffiliateDashboardForViewer\(viewer\.sub, viewer\.role\)/,
  );
  assert.match(bffSource, /proxyAccountGatewayRequest\(\s*sub,/);
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

test("admin worker has KYC R2 archive bucket bindings", () => {
  const wranglerConfig = readAdminFile("wrangler.toml");
  const envSource = readAdminFile("cloudflare-env.d.ts");

  assert.match(wranglerConfig, /^\[\[r2_buckets\]\]$/m);
  assert.match(wranglerConfig, /^binding = ["']KYC_DOCUMENTS_BUCKET["']$/m);
  assert.match(wranglerConfig, /^bucket_name = ["']prophetica-kyc-documents["']$/m);
  assert.match(wranglerConfig, /^binding = ["']KYC_LIVENESS_BUCKET["']$/m);
  assert.match(wranglerConfig, /^bucket_name = ["']prophetica-kyc-liveness["']$/m);
  assert.match(envSource, /KYC_DOCUMENTS_BUCKET\?: R2Bucket/);
  assert.match(envSource, /KYC_LIVENESS_BUCKET\?: R2Bucket/);
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
  assert.ok(
    nextConfig.default.allowedDevOrigins?.some((origin) => origin.length > 0),
  );
  assert.deepEqual(
    nextConfig.parseAllowedDevOrigins(`http://192.168.68.94:${adminPort}`, []),
    ["192.168.68.93", "192.168.68.94"],
  );
  assert.deepEqual(
    nextConfig.parseAllowedDevOrigins(undefined, ["172.20.10.6"]),
    ["192.168.68.93", "172.20.10.6"],
  );
});

test("admin service worker is disabled during local development", () => {
  const registrationSource = readAdminFile(
    "components/service-worker-registration.tsx",
  );
  const workerSource = readAdminFile("public/sw.js");

  assert.match(registrationSource, /process\.env\.NODE_ENV !== "production"/);
  assert.match(registrationSource, /\.getRegistrations\(\)/);
  assert.match(registrationSource, /\.unregister\(\)/);
  assert.match(workerSource, /DEV_HOSTNAMES/);
  assert.match(workerSource, /self\.registration\.unregister\(\)/);
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
  const layoutSource = readAdminFile("app/layout.tsx");

  assert.match(copySource, /adminLocale\s*=\s*"zh-Hant"/);
  assert.match(copySource, /adminCopy/);
  assert.match(copySource, /管理操作入口/);
  assert.match(copySource, /決策支援儀表板/);
  assert.match(overviewSource, /adminCopy\.overview/);
  assert.match(dashboardSource, /adminCopy\.dashboard/);
  assert.match(layoutSource, /adminCopy\.shell/);
});
