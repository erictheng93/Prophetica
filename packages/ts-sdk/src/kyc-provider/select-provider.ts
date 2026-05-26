import type {
  KycAdapterChain,
  KycAdapterStage,
  KycIdentityProvider,
} from "./types.js";
import { KycSdkError } from "./types.js";
import { createIproovMockProvider } from "./iproov-mock-provider.js";
import { createRealIproovProvider } from "./iproov-provider.js";
import { createMockKycProvider } from "./mock-provider.js";

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
  if (opts.sdkToken.startsWith("iproov_sandbox_")) {
    return singleStageChain("iproov");
  }
  if (opts.sdkToken.startsWith("iproov_")) {
    return singleStageChain("iproov");
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
    case "iproov":
      return sdkToken.startsWith("iproov_sandbox_")
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

function singleStageChain(
  name: KycAdapterChain["stages"][number]["name"],
): KycAdapterChain {
  return {
    mountStrategy: "sequential",
    stages: [{ name, required: true, continueAfterSubmit: false }],
  };
}
