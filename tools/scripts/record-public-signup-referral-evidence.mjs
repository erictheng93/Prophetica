#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , signupLogArg, captureLogArg] = process.argv;

if (!signupLogArg || !captureLogArg) {
  console.error(
    "Usage: node tools/scripts/record-public-signup-referral-evidence.mjs <signup-submit-log> <referral-capture-log>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const evidenceDir = resolve(repoRoot, "docs/evidence/public-signup-referral");
const signupLogPath = resolve(process.cwd(), signupLogArg);
const captureLogPath = resolve(process.cwd(), captureLogArg);
const signupLog = readFileSync(signupLogPath);
const captureLog = readFileSync(captureLogPath);
const signupLogText = signupLog.toString("utf8");
const captureLogText = captureLog.toString("utf8");

const requiredSignupMarkers = {
  signup_submit_path: "signup_submit_path=true",
  referral_cookie_submitted: "referral_cookie_submitted=true",
  registration_endpoint_confirmed: "registration_endpoint_confirmed=true",
  durable_login_confirmed: "durable_login_confirmed=true",
};
const requiredCaptureMarkers = {
  referral_capture_confirmed: "referral_capture_confirmed=true",
};
const signupMarkerResults = Object.fromEntries(
  Object.entries(requiredSignupMarkers).map(([key, marker]) => [
    key,
    hasEvidence(signupLogText, marker),
  ]),
);
const captureMarkerResults = Object.fromEntries(
  Object.entries(requiredCaptureMarkers).map(([key, marker]) => [
    key,
    hasEvidence(captureLogText, marker),
  ]),
);
const missingSignupMarkers = Object.entries(signupMarkerResults)
  .filter(([, present]) => !present)
  .map(([key]) => requiredSignupMarkers[key]);
const missingCaptureMarkers = Object.entries(captureMarkerResults)
  .filter(([, present]) => !present)
  .map(([key]) => requiredCaptureMarkers[key]);

if (missingSignupMarkers.length > 0 || missingCaptureMarkers.length > 0) {
  console.error("Public signup referral evidence logs are incomplete.");
  for (const marker of missingSignupMarkers) {
    console.error(`Missing signup marker: ${marker}`);
  }
  for (const marker of missingCaptureMarkers) {
    console.error(`Missing capture marker: ${marker}`);
  }
  process.exit(1);
}

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "Public signup referral attribution",
  ...signupMarkerResults,
  ...captureMarkerResults,
  signup_log: {
    file: basename(signupLogPath),
    sha256: sha256(signupLog),
    bytes: signupLog.byteLength,
  },
  referral_capture_log: {
    file: basename(captureLogPath),
    sha256: sha256(captureLog),
    bytes: captureLog.byteLength,
  },
};

mkdirSync(evidenceDir, { recursive: true });
writeFileSync(
  resolve(evidenceDir, "latest.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
);

console.log(`Public signup referral evidence recorded: ${resolve(evidenceDir, "latest.json")}`);

function hasEvidence(source, marker) {
  return source.includes(marker);
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
