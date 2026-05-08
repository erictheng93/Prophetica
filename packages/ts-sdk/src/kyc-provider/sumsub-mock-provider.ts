import type { KycIdentityProvider } from "./types.js";
import {
  createMockKycProvider,
  type MockKycProviderOptions,
} from "./mock-provider.js";

export type SumsubMockProviderOptions = MockKycProviderOptions;

export function createSumsubMockProvider(
  options: SumsubMockProviderOptions = {},
): KycIdentityProvider {
  return createMockKycProvider(options);
}
