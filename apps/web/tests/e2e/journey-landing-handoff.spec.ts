import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// SPEC-023 plan Task 4.9 — verify the landing/LIFF entry contracts
// described in §6.1 / §6.2. The handoff is server-rendered (static hrefs +
// redirects), so a source-pattern check is the right granularity: spinning
// up apps/landing-sports and apps/liff alongside apps/web inside Playwright
// would multiply the suite runtime and provide no extra signal.
//
// Each rule below maps to a concrete bullet of plan Task 3.3:
//   - landing CTA -> /auth/sign-in?mode=signup
//   - landing does not touch session cookies / next/headers cookies()
//   - LIFF entry redirects to landing only; no in-LINE registration

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(HERE, "..", "..", "..", "..");

function readRepoFile(path: string): string {
  return readFileSync(join(REPO_ROOT, path), "utf8");
}

test("landing-sports signup CTA hands off to apps/web /auth/sign-in?mode=signup", () => {
  const analytics = readRepoFile(
    "apps/landing-sports/app/landing-analytics.tsx",
  );
  expect(analytics).toMatch(
    /const signupDestination = "\/auth\/sign-in\?mode=signup";/,
  );
  expect(analytics).toMatch(/href=\{signupDestination\}/);
});

test("landing-sports nav exposes both signin and signup handoffs to apps/web", () => {
  const nav = readRepoFile("apps/landing-sports/lib/site-nav.tsx");
  expect(nav).toMatch(/href="\/auth\/sign-in"/);
  expect(nav).toMatch(/href="\/auth\/sign-in\?mode=signup"/);
});

test("landing-sports does not set or read session cookies", () => {
  // SPEC-023 §5: landing 不持有 session. Catch any future drift that
  // imports cookies() from next/headers in landing source.
  for (const path of [
    "apps/landing-sports/app/page.tsx",
    "apps/landing-sports/app/layout.tsx",
    "apps/landing-sports/app/landing-analytics.tsx",
    "apps/landing-sports/lib/site-nav.tsx",
  ]) {
    const source = readRepoFile(path);
    expect(source).not.toMatch(/from "next\/headers"/);
    expect(source).not.toMatch(/proph_session/);
    expect(source).not.toMatch(/setViewerSession/);
    expect(source).not.toMatch(/getViewer/);
  }
});

test("LIFF entry redirects to landing only, carries no auth surface", () => {
  const entry = readRepoFile("apps/liff/app/page.tsx");
  expect(entry).toMatch(/import \{ redirect \} from "next\/navigation";/);
  expect(entry).toMatch(/redirect\(landingUrl\);/);
  // SPEC-023 §6.2: LIFF must not host registration, login, KYC, top-up,
  // or order surfaces in this version. The dedicated source test in
  // apps/liff/tests already pins each individually; mirror the most
  // important guard here so a regression trips the journey suite too.
  expect(entry).not.toMatch(/@line\/liff/);
  expect(entry).not.toMatch(/\/api\/auth\/liff\/session/);
});
