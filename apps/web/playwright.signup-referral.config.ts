import { defineConfig, devices } from "@playwright/test";

const WEB_ORIGIN = process.env.WEB_ORIGIN ?? "http://127.0.0.1:3000";
const USER_SERVICE_URL =
  process.env.PUBLIC_SIGNUP_E2E_USER_SERVICE_URL ?? "http://127.0.0.1:43284";
const API_GATEWAY_URL =
  process.env.PUBLIC_SIGNUP_E2E_API_GATEWAY_URL ?? "http://127.0.0.1:43280";

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
      API_GATEWAY_URL,
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
