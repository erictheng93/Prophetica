import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const preflightScript = path.join(repoRoot, "tools/scripts/preflight-spec-022-evidence.mjs");

test("reports missing SPEC-022 evidence collection prerequisites", async () => {
  const root = await makeRoot();

  const failed = spawnSync(process.execPath, [preflightScript, root], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {},
  });

  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /Missing env: NEXT_PUBLIC_LIFF_ID/);
  assert.match(failed.stderr, /Missing env: USER_SERVICE_LINE_LOGIN_CHANNEL_ID/);
  assert.match(failed.stderr, /Missing env: USER_SERVICE_BASE_URL/);
  assert.match(failed.stderr, /Missing input log: \.tmp\/spec-022-liff\.log/);
  assert.match(failed.stderr, /Missing evidence: docs\/evidence\/spec-022-liff\/latest\.json/);
});

test("accepts SPEC-022 evidence collection prerequisites from env files and artifacts", async () => {
  const root = await makeRoot();
  await writeCompletePrerequisites(root);

  const output = execFileSync(process.execPath, [preflightScript, root], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {},
  });

  assert.match(output, /SPEC-022 evidence preflight passed/);
});

test("reports SPEC-022 evidence files missing required true fields", async () => {
  const root = await makeRoot();
  await writeBasePrerequisites(root);
  await mkdir(path.join(root, "docs/evidence/spec-022-liff"), { recursive: true });
  await mkdir(path.join(root, "docs/evidence/spec-022-manual-qa"), { recursive: true });
  await writeFile(
    path.join(root, "docs/evidence/spec-022-liff/latest.json"),
    `${JSON.stringify({ line_channel_configured: true })}\n`,
  );
  await writeFile(
    path.join(root, "docs/evidence/spec-022-manual-qa/latest.json"),
    `${JSON.stringify({ desktop_web_recent_login_approximate_location: true })}\n`,
  );

  const failed = spawnSync(process.execPath, [preflightScript, root], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {},
  });

  assert.equal(failed.status, 1);
  assert.match(
    failed.stderr,
    /Evidence field not true: docs\/evidence\/spec-022-liff\/latest\.json ios_liff_login_success_event/,
  );
  assert.match(
    failed.stderr,
    /Evidence field not true: docs\/evidence\/spec-022-manual-qa\/latest\.json mobile_web_recent_login_approximate_location/,
  );
});

async function makeRoot() {
  return mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-preflight-"));
}

async function writeBasePrerequisites(root) {
  await writeFile(
    path.join(root, ".env"),
    [
      "NEXT_PUBLIC_LIFF_ID=1650000000-example",
      "USER_SERVICE_LINE_LOGIN_CHANNEL_ID=2000000000",
      "USER_SERVICE_BASE_URL=http://127.0.0.1:8080",
      "",
    ].join("\n"),
  );
  await mkdir(path.join(root, ".tmp"), { recursive: true });
  await writeFile(path.join(root, ".tmp/spec-022-liff.log"), "line_channel_configured=true\n");
  await writeFile(
    path.join(root, ".tmp/spec-022-manual-qa.log"),
    "desktop_web_recent_login_approximate_location=true\n",
  );
}

async function writeCompletePrerequisites(root) {
  await writeBasePrerequisites(root);
  await mkdir(path.join(root, "docs/evidence/spec-022-liff"), { recursive: true });
  await mkdir(path.join(root, "docs/evidence/spec-022-manual-qa"), { recursive: true });
  await writeFile(
    path.join(root, "docs/evidence/spec-022-liff/latest.json"),
    `${JSON.stringify({
      line_channel_configured: true,
      ios_liff_login_success_event: true,
      android_liff_login_success_event: true,
      login_surface_liff_confirmed: true,
      user_members_line_id_link_confirmed: true,
    })}\n`,
  );
  await writeFile(
    path.join(root, "docs/evidence/spec-022-manual-qa/latest.json"),
    `${JSON.stringify({
      desktop_web_recent_login_approximate_location: true,
      mobile_web_recent_login_approximate_location: true,
      admin_login_events_rbac_and_audit_checked: true,
      liff_webview_login_event_visible: true,
      unknown_fallback_visible: true,
    })}\n`,
  );
}
