import { defineConfig, devices } from "@playwright/test";

const parsedWebPort = Number.parseInt(process.env.WEB_PORT ?? "3000", 10);
const WEB_PORT = Number.isNaN(parsedWebPort) || parsedWebPort <= 0 ? 3000 : parsedWebPort;
const WEB_ORIGIN = process.env.WEB_ORIGIN ?? `http://localhost:${WEB_PORT}`;
const USER_SERVICE_URL =
  process.env.JOURNEY_E2E_USER_SERVICE_URL ?? "http://127.0.0.1:43284";
const API_GATEWAY_URL =
  process.env.JOURNEY_E2E_API_GATEWAY_URL ?? "http://127.0.0.1:43280";
const KYC_SERVICE_URL =
  process.env.JOURNEY_E2E_KYC_SERVICE_URL ?? "http://127.0.0.1:43285";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 180_000,
  // Next.js dev mode compiles pages lazily on first request. The journey
  // tests touch multiple distinct routes — the default 20s expect timeout
  // is tight against a cold-compile / re-compile cycle.
  expect: {
    timeout: 45_000,
  },
  // The journey suite mocks user-service / api-gateway / kyc-service on
  // fixed ports (43_280 / 43_284 / 43_285) shared across all spec files.
  // Forbid parallel workers so subsequent specs do not collide on those
  // sockets while the previous spec is still tearing down.
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: WEB_ORIGIN,
    trace: "retain-on-failure",
  },
  webServer: {
    command: "pnpm dev",
    env: {
      WEB_PORT: WEB_PORT.toString(),
      PORT: WEB_PORT.toString(),
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
