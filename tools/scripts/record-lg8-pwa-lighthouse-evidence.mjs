#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , lighthouseJsonArg] = process.argv;

if (!lighthouseJsonArg) {
  console.error(
    "Usage: node tools/scripts/record-lg8-pwa-lighthouse-evidence.mjs <staging-lighthouse-json>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const inputPath = resolve(process.cwd(), lighthouseJsonArg);
if (!existsSync(inputPath)) {
  console.error(`Lighthouse JSON not found: ${inputPath}`);
  process.exit(2);
}

const raw = readFileSync(inputPath);
const report = JSON.parse(raw.toString("utf8"));
const requestedUrl = String(report.requestedUrl ?? report.finalDisplayedUrl ?? "");
const finalUrl = String(report.finalDisplayedUrl ?? report.finalUrl ?? "");
const pwaScore = report.categories?.pwa?.score;

const failures = [];
if (!/^https:\/\//i.test(finalUrl || requestedUrl)) {
  failures.push("Lighthouse run must target an HTTPS staging URL.");
}
if (typeof pwaScore !== "number" || pwaScore < 1) {
  failures.push("Lighthouse PWA category score must be 1.0.");
}

for (const [auditId, label] of [
  ["installable-manifest", "installable manifest"],
  ["service-worker", "service worker"],
  ["maskable-icon", "maskable icon"],
]) {
  const score = report.audits?.[auditId]?.score;
  if (typeof score === "number" && score < 1) {
    failures.push(`${label} audit must pass.`);
  }
}

if (failures.length > 0) {
  console.error("LG-8 PWA Lighthouse evidence is incomplete.");
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(1);
}

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "LG-8 PWA staging Lighthouse",
  requested_url: requestedUrl,
  final_url: finalUrl,
  pwa_score: pwaScore,
  source_report: {
    file: basename(inputPath),
    sha256: createHash("sha256").update(raw).digest("hex"),
    bytes: raw.byteLength,
  },
};

const evidenceDir = resolve(repoRoot, "docs/evidence/lg-8-pwa");
mkdirSync(evidenceDir, { recursive: true });
writeFileSync(
  resolve(evidenceDir, "latest.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
);

console.log(`LG-8 PWA evidence recorded: ${resolve(evidenceDir, "latest.json")}`);
