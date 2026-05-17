import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { checkSpec022Complete } from "./check-spec-022-complete.mjs";

async function makeRoot() {
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-spec-022-complete-"));
  await mkdir(path.join(root, "docs/evidence/spec-022-liff"), { recursive: true });
  await mkdir(path.join(root, "docs/evidence/spec-022-manual-qa"), { recursive: true });
  await mkdir(path.join(root, "services/user-service/src"), { recursive: true });
  await writeFile(path.join(root, "docs/spec-022-completion-audit.md"), "# audit\n");
  await writeFile(path.join(root, "services/user-service/src/auth.rs"), "// auth\n");
  return root;
}

test("fails closed while the SPEC-022 manifest is blocked", async () => {
  const root = await makeRoot();
  await writeManifest(root, {
    status: "blocked",
    blocking_items: [
      {
        id: "live-liff",
        reason: "missing device evidence",
      },
    ],
    completed_evidence: [],
  });

  const result = checkSpec022Complete(root);

  assert.equal(result.complete, false);
  assert.deepEqual(result.errors, [
    "SPEC-022 Login Event Intelligence is not complete.",
    "Manifest status: blocked",
    "Blocking items:",
    "- live-liff: missing device evidence",
  ]);
});

test("requires completed evidence artifacts and external evidence files", async () => {
  const root = await makeRoot();
  await writeManifest(root, {
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      {
        id: "missing-artifact",
        artifacts: ["missing/file.rs"],
        evidence: "claimed",
      },
    ],
    required_external_evidence: [
      {
        path: "docs/evidence/spec-022-liff/latest.json",
        required_fields: ["line_channel_configured"],
      },
    ],
  });

  const result = checkSpec022Complete(root);

  assert.equal(result.complete, false);
  assert.deepEqual(result.errors, [
    "SPEC-022 completed evidence missing-artifact references missing artifact: missing/file.rs",
    "Missing SPEC-022 evidence: docs/evidence/spec-022-liff/latest.json",
  ]);
});

test("accepts complete SPEC-022 manifest with real artifacts and evidence booleans", async () => {
  const root = await makeRoot();
  await writeFile(
    path.join(root, "docs/evidence/spec-022-liff/latest.json"),
    `${JSON.stringify({ line_channel_configured: true })}\n`,
  );
  await writeFile(
    path.join(root, "docs/evidence/spec-022-manual-qa/latest.json"),
    `${JSON.stringify({ unknown_fallback_visible: true })}\n`,
  );
  await writeManifest(root, {
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      {
        id: "auth",
        artifacts: ["services/user-service/src/auth.rs"],
        evidence: "tested",
      },
    ],
    required_external_evidence: [
      {
        path: "docs/evidence/spec-022-liff/latest.json",
        required_fields: ["line_channel_configured"],
      },
      {
        path: "docs/evidence/spec-022-manual-qa/latest.json",
        required_fields: ["unknown_fallback_visible"],
      },
    ],
  });

  const result = checkSpec022Complete(root);

  assert.equal(result.complete, true);
  assert.equal(result.completedEvidenceCount, 1);
  assert.deepEqual(result.errors, []);
});

async function writeManifest(root, overrides) {
  await writeFile(
    path.join(root, "docs/spec-022-evidence-manifest.json"),
    `${JSON.stringify(
      {
        last_updated: "2026-05-18",
        scope: "SPEC-022 Login Event Intelligence",
        completion_audit: "docs/spec-022-completion-audit.md",
        required_external_evidence: [],
        ...overrides,
      },
      null,
      2,
    )}\n`,
  );
}
