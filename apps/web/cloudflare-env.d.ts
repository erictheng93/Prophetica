import type { FaqKvNamespace } from "@prophetica/ts-sdk/support-faq";

declare global {
  interface CloudflareEnv {
    FAQ_CONTENT_KV?: FaqKvNamespace;
    KYC_DOCUMENTS_BUCKET?: R2Bucket;
    KYC_LIVENESS_BUCKET?: R2Bucket;
  }
}

export {};
