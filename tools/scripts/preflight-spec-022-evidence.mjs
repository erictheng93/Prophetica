#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const requiredEnv = [
  "NEXT_PUBLIC_LIFF_ID",
  "USER_SERVICE_LINE_LOGIN_CHANNEL_ID",
  "USER_SERVICE_BASE_URL",
];
const requiredInputLogs = [
  ".tmp/spec-022-liff.log",
  ".tmp/spec-022-manual-qa.log",
];
const requiredEvidence = [
  "docs/evidence/spec-022-liff/latest.json",
  "docs/evidence/spec-022-manual-qa/latest.json",
];

export function preflightSpec022Evidence(repoRoot, env = process.env) {
  const envFile = readEnvFile(resolve(repoRoot, ".env"));
  const missing = [];

  for (const key of requiredEnv) {
    const value = env[key] ?? envFile[key];
    if (typeof value !== "string" || value.trim().length === 0) {
      missing.push(`Missing env: ${key}`);
    }
  }

  for (const relativePath of requiredInputLogs) {
    if (!existsSync(resolve(repoRoot, relativePath))) {
      missing.push(`Missing input log: ${relativePath}`);
    }
  }

  for (const relativePath of requiredEvidence) {
    if (!existsSync(resolve(repoRoot, relativePath))) {
      missing.push(`Missing evidence: ${relativePath}`);
    }
  }

  return {
    ok: missing.length === 0,
    missing,
  };
}

function readEnvFile(envPath) {
  if (!existsSync(envPath)) {
    return {};
  }

  const env = {};
  for (const rawLine of readFileSync(envPath, "utf8").split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith("#")) {
      continue;
    }
    const separator = line.indexOf("=");
    if (separator === -1) {
      continue;
    }
    const key = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim();
    if (key.length > 0) {
      env[key] = value;
    }
  }
  return env;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const repoRoot = resolve(process.argv[2] ?? new URL("../..", import.meta.url).pathname);
  const result = preflightSpec022Evidence(repoRoot);
  if (!result.ok) {
    console.error("SPEC-022 evidence preflight failed.");
    for (const item of result.missing) {
      console.error(item);
    }
    process.exit(1);
  }

  console.log("SPEC-022 evidence preflight passed.");
}
