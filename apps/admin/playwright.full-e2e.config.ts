import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 240_000,
  expect: {
    timeout: 30_000,
  },
  use: {
    baseURL: "http://localhost:3002",
    trace: "on-first-retry",
  },
  webServer: {
    command:
      "API_GATEWAY_URL=http://127.0.0.1:43180 ADMIN_INTERNAL_CALL_SECRET=shared-secret-32-bytes-1234567890 MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK=false pnpm dev",
    env: {
      API_GATEWAY_URL: "http://127.0.0.1:43180",
      ADMIN_INTERNAL_CALL_SECRET: "shared-secret-32-bytes-1234567890",
      MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK: "false",
    },
    url: "http://localhost:3002/api/health",
    reuseExistingServer: false,
    timeout: 180_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
