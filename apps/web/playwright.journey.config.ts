import { defineConfig, devices } from "@playwright/test";

const WEB_ORIGIN = process.env.WEB_ORIGIN ?? "http://localhost:3000";
const USER_SERVICE_URL =
  process.env.JOURNEY_E2E_USER_SERVICE_URL ?? "http://127.0.0.1:43284";
const API_GATEWAY_URL =
  process.env.JOURNEY_E2E_API_GATEWAY_URL ?? "http://127.0.0.1:43280";
const KYC_SERVICE_URL =
  process.env.JOURNEY_E2E_KYC_SERVICE_URL ?? "http://127.0.0.1:43285";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 120_000,
  expect: {
    timeout: 20_000,
  },
  use: {
    baseURL: WEB_ORIGIN,
    trace: "on-first-retry",
  },
  webServer: {
    command: "pnpm dev",
    env: {
      USER_SERVICE_URL,
      API_GATEWAY_BASE_URL: API_GATEWAY_URL,
      KYC_SERVICE_BASE_URL: KYC_SERVICE_URL,
      MATCHING_ENGINE_BASE_URL: API_GATEWAY_URL,
      PROPHETICA_DEV_USER_ID: "E2E-USER",
      // SPEC-023 plan v0.1.3: BFF + cookie dev opt-outs. Without these the
      // Next server returns 401 for unauthenticated BFF calls (post-H-1)
      // and writes Secure cookies that browsers may refuse on http://
      // localhost (post-S-2).
      PROPHETICA_ALLOW_DEV_BFF: "1",
      PROPHETICA_INSECURE_COOKIES: "1",
    },
    url: `${WEB_ORIGIN}/api/health`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
