import { getCloudflareContext } from "@opennextjs/cloudflare";
import {
  InMemoryFaqCatalogStore,
  KvFaqCatalogStore,
  type FaqCatalogStore,
  type FaqKvNamespace,
} from "@prophetica/ts-sdk/support-faq";

const FAQ_KV_BINDING = "FAQ_CONTENT_KV";

type RuntimeEnv = CloudflareEnv & {
  FAQ_CONTENT_KV?: FaqKvNamespace;
};

type GlobalWithFaqStore = typeof globalThis & {
  __propheticaLocalFaqStore?: InMemoryFaqCatalogStore;
};

export function getFaqCatalogStore(): FaqCatalogStore {
  const kv = getFaqKvNamespace();
  if (kv) {
    return new KvFaqCatalogStore(kv);
  }

  if (process.env.NODE_ENV === "production") {
    throw new Error(`${FAQ_KV_BINDING} binding is required for FAQ management`);
  }

  const globalStore = globalThis as GlobalWithFaqStore;
  globalStore.__propheticaLocalFaqStore ??= new InMemoryFaqCatalogStore();
  return globalStore.__propheticaLocalFaqStore;
}

function getFaqKvNamespace(): FaqKvNamespace | undefined {
  try {
    const { env } = getCloudflareContext({ async: false });
    return (env as RuntimeEnv)[FAQ_KV_BINDING];
  } catch {
    return undefined;
  }
}
