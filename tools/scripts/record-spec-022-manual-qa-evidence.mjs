#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , manualQaLogArg] = process.argv;

if (!manualQaLogArg) {
  console.error(
    "Usage: node tools/scripts/record-spec-022-manual-qa-evidence.mjs <manual-qa-log>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const evidenceDir = resolve(repoRoot, "docs/evidence/spec-022-manual-qa");
const logPath = resolve(process.cwd(), manualQaLogArg);
const log = readFileSync(logPath);
const logText = log.toString("utf8");
rejectSensitiveEvidence(logText);

const requiredMarkers = {
  desktop_web_recent_login_approximate_location:
    "desktop_web_recent_login_approximate_location=true",
  mobile_web_recent_login_approximate_location:
    "mobile_web_recent_login_approximate_location=true",
  admin_login_events_rbac_and_audit_checked:
    "admin_login_events_rbac_and_audit_checked=true",
  liff_webview_login_event_visible: "liff_webview_login_event_visible=true",
  unknown_fallback_visible: "unknown_fallback_visible=true",
};

const markerResults = Object.fromEntries(
  Object.entries(requiredMarkers).map(([key, marker]) => [
    key,
    hasEvidence(logText, marker),
  ]),
);
const missingMarkers = Object.entries(markerResults)
  .filter(([, present]) => !present)
  .map(([key]) => requiredMarkers[key]);

if (missingMarkers.length > 0) {
  console.error("SPEC-022 manual QA evidence log is incomplete.");
  for (const marker of missingMarkers) {
    console.error(`Missing marker: ${marker}`);
  }
  process.exit(1);
}

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "SPEC-022 manual QA",
  ...markerResults,
  manual_qa_log: {
    file: basename(logPath),
    sha256: sha256(log),
    bytes: log.byteLength,
  },
};

mkdirSync(evidenceDir, { recursive: true });
writeFileSync(
  resolve(evidenceDir, "latest.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
);

console.log(
  `SPEC-022 manual QA evidence recorded: ${resolve(evidenceDir, "latest.json")}`,
);

function hasEvidence(source, marker) {
  return source.includes(marker);
}

function rejectSensitiveEvidence(source) {
  const forbiddenPatterns = [
    /\bid_token\s*[=:]/i,
    /\baccess_token\s*[=:]/i,
    /\bsession_token\s*[=:]/i,
    /\brefresh_token\s*[=:]/i,
    /\bauthorization:\s*bearer\b/i,
    /\bproph_user_service_session\s*=/i,
    /\bpassword\s*[=:]/i,
    /\botp\s*[=:]/i,
  ];
  for (const pattern of forbiddenPatterns) {
    if (pattern.test(source)) {
      console.error(
        "SPEC-022 manual QA evidence log appears to contain a token, password, OTP, cookie, or authorization secret. Redact it before recording evidence.",
      );
      process.exit(1);
    }
  }
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
