#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , webauthnLogArg] = process.argv;

if (!webauthnLogArg) {
  console.error(
    "Usage: node tools/scripts/record-webauthn-passkey-evidence.mjs <webauthn-browser-e2e-log>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const evidenceDir = resolve(repoRoot, "docs/evidence/webauthn-passkey");
const logPath = resolve(process.cwd(), webauthnLogArg);
const log = readFileSync(logPath);
const logText = log.toString("utf8");

const requiredMarkers = {
  registration_round_trip: "registration_round_trip=true",
  registration_replay_rejected: "registration_replay_rejected=true",
  assertion_round_trip: "assertion_round_trip=true",
  assertion_replay_rejected: "assertion_replay_rejected=true",
  wrong_origin_rejected: "wrong_origin_rejected=true",
  unknown_credential_rejected: "unknown_credential_rejected=true",
  revoked_credential_rejected: "revoked_credential_rejected=true",
  fresh_hardware_mfa_session: "fresh_hardware_mfa_session=true",
  admin_step_up_fresh_accepts: "admin_step_up_fresh_accepts=true",
  admin_step_up_stale_rejects: "admin_step_up_stale_rejects=true",
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
  console.error("WebAuthn / passkey evidence log is incomplete.");
  for (const marker of missingMarkers) {
    console.error(`Missing marker: ${marker}`);
  }
  process.exit(1);
}

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "WebAuthn / passkey P0 browser ceremony",
  contract: "docs/webauthn-passkey-p0-contract.md",
  ...markerResults,
  browser_log: {
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

console.log(`WebAuthn / passkey evidence recorded: ${resolve(evidenceDir, "latest.json")}`);

function hasEvidence(source, marker) {
  return source.includes(marker);
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
