#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const terraformPath = resolve(repoRoot, "infra/cloudflare/zero-trust/admin-access.tf");
const source = readFileSync(terraformPath, "utf8");

const checks = [
  {
    name: "login bypass application exists",
    pattern: /resource\s+"cloudflare_zero_trust_access_application"\s+"admin_login_bypass"/,
  },
  {
    name: "login bypass covers /login*",
    pattern: /domain\s*=\s*"\$\{var\.admin_hostname\}\/login\*"/,
  },
  {
    name: "affiliate portal bypass application exists",
    pattern: /resource\s+"cloudflare_zero_trust_access_application"\s+"affiliate_portal_bypass"/,
  },
  {
    name: "affiliate bypass covers /me/affiliate*",
    pattern: /domain\s*=\s*"\$\{var\.admin_hostname\}\/me\/affiliate\*"/,
  },
  {
    name: "public bypass policy uses bypass decision",
    pattern: /resource\s+"cloudflare_zero_trust_access_policy"\s+"public_bypass"[\s\S]*?decision\s*=\s*"bypass"/,
  },
  {
    name: "internal admin application exists",
    pattern: /resource\s+"cloudflare_zero_trust_access_application"\s+"admin_internal"/,
  },
  {
    name: "internal policy uses allow decision",
    pattern: /resource\s+"cloudflare_zero_trust_access_policy"\s+"admin_internal_sso"[\s\S]*?decision\s*=\s*"allow"/,
  },
  {
    name: "internal policy requires security key MFA",
    pattern: /allowed_authenticators\s*=\s*\[\s*"security_key"\s*\]/,
  },
  {
    name: "internal policy uses 30 minute session",
    pattern: /session_duration\s*=\s*"30m"/,
  },
];

const failures = checks.filter((check) => !check.pattern.test(source));

if (failures.length > 0) {
  console.error("Cloudflare admin Access policy validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure.name}`);
  }
  process.exit(1);
}

console.log(`Cloudflare admin Access policy validation passed (${checks.length} checks).`);

