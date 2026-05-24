import type { KycIdentityProvider } from "./types.js";
import { createMockKycProvider } from "./mock-provider.js";

export function createIproovMockProvider(): KycIdentityProvider {
  const provider = createMockKycProvider({ autoSubmitAfterMs: 10 });
  return {
    ...provider,
    name: "iproov",
  };
}
