import type { KycIdentityProvider } from "./types.js";
import { KycSdkError } from "./types.js";

export function createRealSumsubProvider(): KycIdentityProvider {
  return {
    name: "sumsub",
    async mount() {
      throw new KycSdkError(
        "not_implemented",
        "Real SumSub provider is not implemented yet. Pending PoC-B verification and SumSub sandbox account. See SPEC-005a §3.2 and SPEC-005 §0.3.",
      );
    },
  };
}
