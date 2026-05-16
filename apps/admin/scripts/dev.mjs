#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootEnvPath = resolve(__dirname, "..", "..", "..", ".env");

const env = { ...process.env };
const envDefaults = [
  "ADMIN_PORT",
  "API_GATEWAY_PORT",
  "USER_SERVICE_PORT",
  "API_GATEWAY_URL",
  "USER_SERVICE_URL",
  "ADMIN_INTERNAL_CALL_SECRET",
  "MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK",
  "INTERNAL_CALL_HMAC_SECRET",
];

if (existsSync(rootEnvPath)) {
  const lines = readFileSync(rootEnvPath, "utf8").split(/\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const assignment = trimmed.match(/^([A-Z0-9_]+)=(.*)$/);
    if (!assignment) {
      continue;
    }

    const [, key, rawValue] = assignment;
    if (!envDefaults.includes(key)) {
      continue;
    }

    if (env[key] == null) {
      env[key] = stripEnvQuotes(rawValue);
    }
  }
}

if (!env.API_GATEWAY_URL) {
  const port = Number.parseInt(env.API_GATEWAY_PORT ?? "8080", 10);
  env.API_GATEWAY_URL = `http://127.0.0.1:${Number.isNaN(port) ? 8080 : port}`;
}

if (!env.USER_SERVICE_URL) {
  const port = Number.parseInt(env.USER_SERVICE_PORT ?? "8084", 10);
  env.USER_SERVICE_URL = `http://127.0.0.1:${Number.isNaN(port) ? 8084 : port}`;
}

const adminPort = Number.parseInt(env.ADMIN_PORT ?? "3002", 10);
const normalizedAdminPort = Number.isNaN(adminPort) || adminPort <= 0 ? 3002 : adminPort;

const child = spawnSync("next", ["dev", "--webpack", "-p", `${normalizedAdminPort}`], {
  stdio: "inherit",
  env,
  shell: process.platform === "win32",
});

process.exit(child.status ?? 1);

function stripEnvQuotes(value) {
  const match = value.match(/^(["'])([\s\S]*)\1$/);
  return match ? match[2] : value;
}
