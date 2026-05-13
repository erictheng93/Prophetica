import type { FaqKvNamespace } from "@prophetica/ts-sdk/support-faq";

declare global {
  interface CloudflareEnv {
    FAQ_CONTENT_KV?: FaqKvNamespace;
  }
}

export {};
