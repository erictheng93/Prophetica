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
  await mkdir(path.join(root, "docs/evidence/spec-022-liff"), { recursive: true });
  await mkdir(path.join(root, "docs/evidence/spec-022-manual-qa"), { recursive: true });
  await writeFile(path.join(root, "docs/evidence/spec-022-liff/latest.json"), "{}\n");
  await writeFile(path.join(root, "docs/evidence/spec-022-manual-qa/latest.json"), "{}\n");

  const output = execFileSync(process.execPath, [preflightScript, root], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {},
  });

  assert.match(output, /SPEC-022 evidence preflight passed/);
});

async function makeRoot() {
  return mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-preflight-"));
}
