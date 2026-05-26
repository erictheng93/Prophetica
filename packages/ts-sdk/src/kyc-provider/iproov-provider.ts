import type { KycIdentityProvider } from "./types.js";
import { KycSdkError } from "./types.js";

export function createRealIproovProvider(): KycIdentityProvider {
  return {
    name: "iproov",
    async mount() {
      throw new KycSdkError(
        "not_implemented",
        "Real iProov provider is not implemented yet. Blocked on iProov sandbox credentials and verified mount behavior.",
      );
    },
  };
}
