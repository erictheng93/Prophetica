import type { KycIdentityProvider } from "./types.js";
import { KycSdkError } from "./types.js";
import { createMockKycProvider } from "./mock-provider.js";
import { createSumsubMockProvider } from "./sumsub-mock-provider.js";
import { createRealSumsubProvider } from "./sumsub-provider.js";

export interface SelectKycProviderOptions {
  sdkToken: string;
  override?: KycIdentityProvider;
}

export function selectKycProvider(
  opts: SelectKycProviderOptions,
): KycIdentityProvider {
  if (opts.override) {
    return opts.override;
  }
  if (opts.sdkToken.startsWith("sumsub_sandbox_")) {
    return createSumsubMockProvider();
  }
  if (opts.sdkToken.startsWith("sumsub_")) {
    return createRealSumsubProvider();
  }
  if (opts.sdkToken.startsWith("mock_")) {
    return createMockKycProvider();
  }
  throw new KycSdkError(
    "unknown_sdk_token_prefix",
    "Cannot resolve KYC provider for sdk_token; unsupported prefix.",
  );
}
