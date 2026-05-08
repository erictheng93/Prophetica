import type {
  KycAdapterChain,
  KycAdapterStage,
  KycIdentityProvider,
} from "./types.js";
import { KycSdkError } from "./types.js";
import { createIproovMockProvider } from "./iproov-mock-provider.js";
import { createRealIproovProvider } from "./iproov-provider.js";
import { createMockKycProvider } from "./mock-provider.js";
import { createSumsubMockProvider } from "./sumsub-mock-provider.js";
import { createRealSumsubProvider } from "./sumsub-provider.js";

export interface SelectKycProviderOptions {
  sdkToken: string;
  override?: KycIdentityProvider;
}

export function selectKycProvider(
  opts: SelectKycProviderOptions,
): KycAdapterChain {
  if (opts.override) {
    return singleStageChain(opts.override.name);
  }
  if (opts.sdkToken.startsWith("sumsub_sandbox_")) {
    return sumsubChain(opts.sdkToken);
  }
  if (opts.sdkToken.startsWith("sumsub_")) {
    return sumsubChain(opts.sdkToken);
  }
  if (opts.sdkToken.startsWith("mock_")) {
    return singleStageChain("mock");
  }
  throw new KycSdkError(
    "unknown_sdk_token_prefix",
    "Cannot resolve KYC provider for sdk_token; unsupported prefix.",
  );
}

export function createKycProviderForStage(
  stage: KycAdapterStage,
  sdkToken: string,
): KycIdentityProvider {
  switch (stage.name) {
    case "sumsub":
      return sdkToken.startsWith("sumsub_sandbox_")
        ? createSumsubMockProvider()
        : createRealSumsubProvider();
    case "iproov":
      return sdkToken.startsWith("sumsub_sandbox_")
        ? createIproovMockProvider()
        : createRealIproovProvider();
    case "mock":
      return createMockKycProvider();
    case "onfido":
    case "jumio":
      throw new KycSdkError(
        "not_implemented",
        `KYC provider ${stage.name} is not implemented yet.`,
      );
  }
}

function sumsubChain(sdkToken: string): KycAdapterChain {
  if (sdkToken.includes("_TIER_3_")) {
    return {
      mountStrategy: "sequential",
      stages: [
        { name: "sumsub", required: true, continueAfterSubmit: true },
        { name: "iproov", required: true, continueAfterSubmit: false },
      ],
    };
  }
  return singleStageChain("sumsub");
}

function singleStageChain(
  name: KycAdapterChain["stages"][number]["name"],
): KycAdapterChain {
  return {
    mountStrategy: "sequential",
    stages: [{ name, required: true, continueAfterSubmit: false }],
  };
}
