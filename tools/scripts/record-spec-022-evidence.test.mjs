import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const liffScript = path.join(repoRoot, "tools/scripts/record-spec-022-liff-evidence.mjs");
const manualQaScript = path.join(
  repoRoot,
  "tools/scripts/record-spec-022-manual-qa-evidence.mjs",
);
const liffEvidenceDir = path.join(repoRoot, "docs/evidence/spec-022-liff");
const manualQaEvidenceDir = path.join(
  repoRoot,
  "docs/evidence/spec-022-manual-qa",
);

test("records SPEC-022 LIFF evidence only when all markers are present", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-liff-"));
  const incompleteLog = path.join(root, "incomplete.log");
  const completeLog = path.join(root, "complete.log");
  await writeFile(incompleteLog, "line_channel_configured=true\n");
  await writeFile(
    completeLog,
    [
      "line_channel_configured=true",
      "ios_liff_login_success_event=true",
      "android_liff_login_success_event=true",
      "login_surface_liff_confirmed=true",
      "user_members_line_id_link_confirmed=true",
      "",
    ].join("\n"),
  );

  const failed = spawnSync(process.execPath, [liffScript, incompleteLog], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /Missing marker: ios_liff_login_success_event=true/);

  execFileSync(process.execPath, [liffScript, completeLog], { cwd: repoRoot });
  const evidence = JSON.parse(
    await readFile(path.join(liffEvidenceDir, "latest.json"), "utf8"),
  );
  assert.equal(evidence.line_channel_configured, true);
  assert.equal(evidence.ios_liff_login_success_event, true);
  assert.equal(evidence.android_liff_login_success_event, true);
  assert.equal(evidence.login_surface_liff_confirmed, true);
  assert.equal(evidence.user_members_line_id_link_confirmed, true);
  assert.equal(evidence.liff_device_login_log.file, "complete.log");

  await cleanEvidence();
});

test("rejects SPEC-022 LIFF evidence logs containing token material", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-liff-secret-"));
  const logPath = path.join(root, "secret.log");
  await writeFile(
    logPath,
    [
      "line_channel_configured=true",
      "ios_liff_login_success_event=true",
      "android_liff_login_success_event=true",
      "login_surface_liff_confirmed=true",
      "user_members_line_id_link_confirmed=true",
      "id_token=redacted-but-should-not-be-present",
      "",
    ].join("\n"),
  );

  const failed = spawnSync(process.execPath, [liffScript, logPath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /appears to contain a token/);

  await cleanEvidence();
});

test("records SPEC-022 manual QA evidence only when all markers are present", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-qa-"));
  const incompleteLog = path.join(root, "incomplete.log");
  const completeLog = path.join(root, "complete.log");
  await writeFile(incompleteLog, "desktop_web_recent_login_approximate_location=true\n");
  await writeFile(
    completeLog,
    [
      "desktop_web_recent_login_approximate_location=true",
      "mobile_web_recent_login_approximate_location=true",
      "admin_login_events_rbac_and_audit_checked=true",
      "liff_webview_login_event_visible=true",
      "unknown_fallback_visible=true",
      "",
    ].join("\n"),
  );

  const failed = spawnSync(process.execPath, [manualQaScript, incompleteLog], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(failed.status, 1);
  assert.match(
    failed.stderr,
    /Missing marker: mobile_web_recent_login_approximate_location=true/,
  );

  execFileSync(process.execPath, [manualQaScript, completeLog], { cwd: repoRoot });
  const evidence = JSON.parse(
    await readFile(path.join(manualQaEvidenceDir, "latest.json"), "utf8"),
  );
  assert.equal(evidence.desktop_web_recent_login_approximate_location, true);
  assert.equal(evidence.mobile_web_recent_login_approximate_location, true);
  assert.equal(evidence.admin_login_events_rbac_and_audit_checked, true);
  assert.equal(evidence.liff_webview_login_event_visible, true);
  assert.equal(evidence.unknown_fallback_visible, true);
  assert.equal(evidence.manual_qa_log.file, "complete.log");

  await cleanEvidence();
});

test("rejects SPEC-022 manual QA logs containing credentials", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-qa-secret-"));
  const logPath = path.join(root, "secret.log");
  await writeFile(
    logPath,
    [
      "desktop_web_recent_login_approximate_location=true",
      "mobile_web_recent_login_approximate_location=true",
      "admin_login_events_rbac_and_audit_checked=true",
      "liff_webview_login_event_visible=true",
      "unknown_fallback_visible=true",
      "password=redacted-but-should-not-be-present",
      "",
    ].join("\n"),
  );

  const failed = spawnSync(process.execPath, [manualQaScript, logPath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /appears to contain a token/);

  await cleanEvidence();
});

async function cleanEvidence() {
  await mkdir(path.join(repoRoot, "docs/evidence"), { recursive: true });
  await rm(liffEvidenceDir, { recursive: true, force: true });
  await rm(manualQaEvidenceDir, { recursive: true, force: true });
}
