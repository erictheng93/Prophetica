#!/usr/bin/env node
import { mkdirSync } from "node:fs";
import { access, constants, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { delimiter, resolve } from "node:path";
import process from "node:process";

const repoRoot = resolve(new URL("../..", import.meta.url).pathname);
const terraformDir = resolve(repoRoot, "infra/cloudflare/zero-trust");
const tmpDir = resolve(repoRoot, ".tmp");
const planLogPath = resolve(tmpDir, "cloudflare-admin-access-plan.log");
const applyLogPath = resolve(tmpDir, "cloudflare-admin-access-apply.log");
const recorderPath = resolve(
  repoRoot,
  "tools/scripts/record-cloudflare-admin-access-evidence.mjs",
);

const usage = [
  "Usage:",
  "  rtk node tools/scripts/apply-cloudflare-admin-access-evidence.mjs --apply",
  "",
  "Required environment:",
  "  CLOUDFLARE_ACCOUNT_ID",
  "  CLOUDFLARE_API_TOKEN",
  "  TF_VAR_admin_app_domain",
  "  TF_VAR_admin_allowed_email_domains",
  "  TF_VAR_admin_breakglass_email",
  "  TF_VAR_internal_service_token_name",
  "",
  "This script runs terraform/tofu init, plan, apply, then records",
  "docs/evidence/cloudflare-admin-access/latest.json from the captured logs.",
].join("\n");

const requiredTargetVariables = [
  "CLOUDFLARE_ACCOUNT_ID",
  "CLOUDFLARE_API_TOKEN",
  "TF_VAR_admin_app_domain",
  "TF_VAR_admin_allowed_email_domains",
  "TF_VAR_admin_breakglass_email",
  "TF_VAR_internal_service_token_name",
];

async function executableExists(name) {
  const paths = (process.env.PATH ?? "").split(delimiter).filter(Boolean);

  for (const path of paths) {
    try {
      await access(resolve(path, name), constants.X_OK);
      return true;
    } catch {
      // Continue scanning PATH.
    }
  }

  return false;
}

function hasNonEmptyEnv(name) {
  const value = process.env[name];
  return typeof value === "string" && value.trim().length > 0;
}

function run(command, args, options = {}) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd ?? repoRoot,
      env: {
        ...process.env,
        CHECKPOINT_DISABLE: "1",
        TF_IN_AUTOMATION: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];

    child.stdout.on("data", (chunk) => {
      process.stdout.write(chunk);
      stdout.push(chunk);
    });
    child.stderr.on("data", (chunk) => {
      process.stderr.write(chunk);
      stderr.push(chunk);
    });
    child.on("error", reject);
    child.on("close", (code) => {
      const output = Buffer.concat([...stdout, ...stderr]).toString("utf8");
      if (options.logPath) {
        writeFile(options.logPath, output).then(
          () => {
            if (code === 0) {
              resolveRun();
            } else {
              reject(new Error(`${command} ${args.join(" ")} exited ${code}`));
            }
          },
          reject,
        );
        return;
      }
      if (code === 0) {
        resolveRun();
      } else {
        reject(new Error(`${command} ${args.join(" ")} exited ${code}`));
      }
    });
  });
}

if (!process.argv.includes("--apply")) {
  console.error(usage);
  process.exit(2);
}

const missingEnv = requiredTargetVariables.filter((name) => !hasNonEmptyEnv(name));
if (missingEnv.length > 0) {
  console.error(`Missing required environment variables: ${missingEnv.join(", ")}`);
  process.exit(1);
}

const terraformAvailable = await executableExists("terraform");
const tofuAvailable = await executableExists("tofu");
const binary = terraformAvailable ? "terraform" : tofuAvailable ? "tofu" : null;

if (!binary) {
  console.error("Neither terraform nor tofu is available on PATH");
  process.exit(1);
}

mkdirSync(tmpDir, { recursive: true });

await run(binary, ["init", "-input=false", "-no-color"], { cwd: terraformDir });
await run(binary, ["plan", "-input=false", "-no-color", "-out=.terraform/admin-access.tfplan"], {
  cwd: terraformDir,
  logPath: planLogPath,
});
await run(binary, [
  "apply",
  "-input=false",
  "-no-color",
  "-auto-approve",
  ".terraform/admin-access.tfplan",
], {
  cwd: terraformDir,
  logPath: applyLogPath,
});
await run(process.execPath, [recorderPath, planLogPath, applyLogPath]);
