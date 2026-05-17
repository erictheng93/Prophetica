#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , liffLogArg] = process.argv;

if (!liffLogArg) {
  console.error(
    "Usage: node tools/scripts/record-spec-022-liff-evidence.mjs <liff-device-login-log>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const evidenceDir = resolve(repoRoot, "docs/evidence/spec-022-liff");
const logPath = resolve(process.cwd(), liffLogArg);
const log = readFileSync(logPath);
const logText = log.toString("utf8");

const requiredMarkers = {
  line_channel_configured: "line_channel_configured=true",
  ios_liff_login_success_event: "ios_liff_login_success_event=true",
  android_liff_login_success_event: "android_liff_login_success_event=true",
  login_surface_liff_confirmed: "login_surface_liff_confirmed=true",
  user_members_line_id_link_confirmed: "user_members_line_id_link_confirmed=true",
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
  console.error("SPEC-022 LIFF evidence log is incomplete.");
  for (const marker of missingMarkers) {
    console.error(`Missing marker: ${marker}`);
  }
  process.exit(1);
}

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "SPEC-022 LIFF identity bridge",
  ...markerResults,
  liff_device_login_log: {
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

console.log(`SPEC-022 LIFF evidence recorded: ${resolve(evidenceDir, "latest.json")}`);

function hasEvidence(source, marker) {
  return source.includes(marker);
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
