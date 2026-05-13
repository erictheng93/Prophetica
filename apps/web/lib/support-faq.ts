import { getCloudflareContext } from "@opennextjs/cloudflare";
import {
  KvFaqCatalogStore,
  backfillFaqCatalogMissingLocalizedText,
  faqCatalogToPublicSections,
  type FaqCatalogStore,
  type FaqKvNamespace,
  type FaqLocale,
  type PublicFaqSection,
} from "@prophetica/ts-sdk/support-faq";

const FAQ_KV_BINDING = "FAQ_CONTENT_KV";

type RuntimeEnv = CloudflareEnv & {
  FAQ_CONTENT_KV?: FaqKvNamespace;
};

interface LoadManagedFaqSectionsInput {
  locale: FaqLocale;
  store?: FaqCatalogStore | null;
}

export async function loadManagedFaqSections({
  locale,
  store = getFaqCatalogStore(),
}: LoadManagedFaqSectionsInput): Promise<PublicFaqSection[] | null> {
  if (!store) return null;

  try {
    const catalog = await store.load();
    return catalog
      ? faqCatalogToPublicSections(
          backfillFaqCatalogMissingLocalizedText(catalog),
          locale,
        )
      : null;
  } catch {
    return null;
  }
}

function getFaqCatalogStore(): FaqCatalogStore | null {
  const kv = getFaqKvNamespace();
  return kv ? new KvFaqCatalogStore(kv) : null;
}

function getFaqKvNamespace(): FaqKvNamespace | undefined {
  try {
    const { env } = getCloudflareContext({ async: false });
    return (env as RuntimeEnv)[FAQ_KV_BINDING];
  } catch {
    return undefined;
  }
}
