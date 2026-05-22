#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = resolve(__dirname, "..", "..");

const mode = process.argv[2] ?? "mvp";
const env = { ...process.env };
const loadedEnvPath = loadRootEnv(env);
applyDerivedDefaults(env);

const commandSets = {
  "core-api": [
    {
      name: "core-api",
      command: "cargo",
      args: ["run", "-p", "core-api", "--locked"],
    },
  ],
  apps: [
    {
      name: "apps",
      command: "pnpm",
      args: ["run", "dev:apps"],
    },
  ],
  mvp: [
    {
      name: "core-api",
      command: "cargo",
      args: ["run", "-p", "core-api", "--locked"],
    },
    {
      name: "apps",
      command: "pnpm",
      args: ["run", "dev:apps"],
    },
  ],
};

const commands = commandSets[mode];
if (!commands) {
  console.error(
    `Unsupported dev mode "${mode}". Use one of: ${Object.keys(commandSets).join(", ")}`,
  );
  process.exit(2);
}

if (loadedEnvPath) {
  console.log(`Loaded ${relative(root, loadedEnvPath)} for local dev.`);
}

const children = commands.map(startCommand);
let shuttingDown = false;
let exitCode = 0;

process.on("SIGINT", () => shutdown("SIGINT", 130));
process.on("SIGTERM", () => shutdown("SIGTERM", 143));

function startCommand({ name, command, args }) {
  const child = spawn(command, args, {
    cwd: root,
    env,
    shell: process.platform === "win32",
    stdio: ["inherit", "pipe", "pipe"],
  });

  pipeLines(name, child.stdout, process.stdout);
  pipeLines(name, child.stderr, process.stderr);

  child.on("error", (error) => {
    console.error(`[${name}] failed to start: ${error.message}`);
    shutdown("SIGTERM", 1);
  });

  child.on("exit", (code, signal) => {
    if (shuttingDown) {
      return;
    }
    const status = signal ? `signal ${signal}` : `exit ${code ?? 1}`;
    console.error(`[${name}] stopped with ${status}; shutting down local dev.`);
    shutdown("SIGTERM", code ?? 1);
  });

  return child;
}

function shutdown(signal, code) {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  exitCode = code;

  for (const child of children) {
    if (!child.killed) {
      child.kill(signal);
    }
  }

  setTimeout(() => process.exit(exitCode), 5_000).unref();
}

function pipeLines(name, stream, writer) {
  let buffer = "";
  stream.on("data", (chunk) => {
    buffer += chunk.toString();
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      writer.write(`[${name}] ${line}\n`);
    }
  });
  stream.on("end", () => {
    if (buffer.length > 0) {
      writer.write(`[${name}] ${buffer}\n`);
    }
  });
}

function loadRootEnv(targetEnv) {
  for (const candidate of [".env", ".env.example"]) {
    const envPath = resolve(root, candidate);
    if (existsSync(envPath)) {
      loadEnvFile(envPath, targetEnv);
      return envPath;
    }
  }
  return null;
}

function loadEnvFile(envPath, targetEnv) {
  const lines = readFileSync(envPath, "utf8").split(/\n/);
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
    if (targetEnv[key] == null) {
      targetEnv[key] = stripEnvQuotes(rawValue);
    }
  }
}

function applyDerivedDefaults(targetEnv) {
  const apiGatewayUrl = serviceUrl(targetEnv.API_GATEWAY_PORT, 8080);
  const walletLedgerUrl = serviceUrl(
    targetEnv.WALLET_LEDGER_SERVICE_PORT,
    8086,
  );

  targetEnv.API_GATEWAY_BASE_URL ??= targetEnv.API_GATEWAY_URL ?? apiGatewayUrl;
  targetEnv.API_GATEWAY_URL ??= targetEnv.API_GATEWAY_BASE_URL;
  targetEnv.WALLET_LEDGER_BASE_URL ??=
    targetEnv.LEDGER_SERVICE_URL ?? walletLedgerUrl;
  targetEnv.WALLET_LEDGER_DATABASE_URL ??= targetEnv.DATABASE_URL;
}

function serviceUrl(portValue, fallbackPort) {
  const parsedPort = Number.parseInt(portValue ?? "", 10);
  const port =
    Number.isNaN(parsedPort) || parsedPort <= 0 ? fallbackPort : parsedPort;
  return `http://127.0.0.1:${port}`;
}

function stripEnvQuotes(value) {
  const match = value.match(/^(["'])([\s\S]*)\1$/);
  return match ? match[2] : value;
}
