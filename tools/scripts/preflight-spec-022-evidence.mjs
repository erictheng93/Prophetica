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

export function preflightSpec022Evidence(repoRoot, env = process.env) {
  const envFile = readEnvFile(resolve(repoRoot, ".env"));
  const manifest = readManifest(repoRoot);
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

  for (const requirement of manifest.required_external_evidence) {
    const relativePath = requirement.path;
    const evidencePath = resolve(repoRoot, relativePath);
    if (!existsSync(evidencePath)) {
      missing.push(`Missing evidence: ${relativePath}`);
      continue;
    }

    const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
    for (const field of requirement.required_fields) {
      if (evidence[field] !== true) {
        missing.push(`Evidence field not true: ${relativePath} ${field}`);
      }
    }
  }

  return {
    ok: missing.length === 0,
    missing,
  };
}

function readManifest(repoRoot) {
  const manifestPath = resolve(repoRoot, "docs/spec-022-evidence-manifest.json");
  if (existsSync(manifestPath)) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    return {
      required_external_evidence: Array.isArray(manifest.required_external_evidence)
        ? manifest.required_external_evidence
        : [],
    };
  }

  return {
    required_external_evidence: [
      {
        path: "docs/evidence/spec-022-liff/latest.json",
        required_fields: [
          "line_channel_configured",
          "ios_liff_login_success_event",
          "android_liff_login_success_event",
          "login_surface_liff_confirmed",
          "user_members_line_id_link_confirmed",
        ],
      },
      {
        path: "docs/evidence/spec-022-manual-qa/latest.json",
        required_fields: [
          "desktop_web_recent_login_approximate_location",
          "mobile_web_recent_login_approximate_location",
          "admin_login_events_rbac_and_audit_checked",
          "liff_webview_login_event_visible",
          "unknown_fallback_visible",
        ],
      },
    ],
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
