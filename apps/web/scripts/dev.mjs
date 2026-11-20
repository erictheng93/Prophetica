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
  "WEB_PORT",
  "PORT",
  "API_GATEWAY_PORT",
  "USER_SERVICE_PORT",
  "API_GATEWAY_URL",
  "API_GATEWAY_BASE_URL",
  "USER_SERVICE_URL",
  "KYC_SERVICE_URL",
  "KYC_SERVICE_BASE_URL",
  "MATCHING_ENGINE_BASE_URL",
  "WALLET_LEDGER_BASE_URL",
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

const rawPort = env.WEB_PORT ?? env.PORT ?? "3000";
const webPort = Number.parseInt(rawPort, 10);
const normalizedWebPort = Number.isNaN(webPort) || webPort <= 0 ? 3000 : webPort;

const child = spawnSync("next", ["dev", "--webpack", "-p", `${normalizedWebPort}`], {
  stdio: "inherit",
  env,
  shell: process.platform === "win32",
});

process.exit(child.status ?? 1);

function stripEnvQuotes(value) {
  const match = value.match(/^(["'])([\s\S]*)\1$/);
  return match ? match[2] : value;
}
