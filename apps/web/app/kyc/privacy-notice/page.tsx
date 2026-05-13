import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

// SPEC-005 §8.1 mandate — "採集前必有單獨頁面說明：我們收什麼、為什麼、保存多久"。
// This page satisfies that requirement. It must be linked from every KYC
// form (Tier 1 / 2 / 3) and read-acknowledged before submission.

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.privacyNotice.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: true, follow: true },
  };
}

interface PrivacyRow {
  key:
    | "legalName"
    | "birthday"
    | "documentId"
    | "documentImage"
    | "selfie"
    | "address"
    | "income"
    | "funds";
}

const matrix: PrivacyRow[] = [
  { key: "legalName" },
  { key: "birthday" },
  { key: "documentId" },
  { key: "documentImage" },
  { key: "selfie" },
  { key: "address" },
  { key: "income" },
  { key: "funds" },
];

const rightKeys = [
  "query",
  "rectify",
  "delete",
  "portability",
  "object",
] as const;

type PrivacyNoticeCopy = Awaited<
  ReturnType<typeof getTranslations<"kyc.privacyNotice">>
>;

function PrivacyMatrix({ t }: { t: PrivacyNoticeCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("matrix.heading")}
        </h2>

        <div className="space-y-4">
          {matrix.map((row) => (
            <article
              key={row.key}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <header className="flex flex-wrap items-baseline gap-3">
                <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {t(`matrix.rows.${row.key}.field`)}
                </h3>
                <span className="rounded-proph-pill border border-proph-hairline bg-proph-canvas-parchment px-3 py-1 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t(`matrix.rows.${row.key}.collected`)}
                </span>
              </header>

              <dl className="mt-4 grid grid-cols-1 gap-4 md:grid-cols-3">
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("matrix.labels.purpose")}
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t(`matrix.rows.${row.key}.purpose`)}
                  </dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("matrix.labels.retention")}
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t.rich(`matrix.rows.${row.key}.retention`, {
                      strong: (chunks) => (
                        <strong className="font-semibold text-proph-ink">
                          {chunks}
                        </strong>
                      ),
                    })}
                  </dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("matrix.labels.access")}
                  </dt>
                  <dd className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t.rich(`matrix.rows.${row.key}.access`, {
                      strong: (chunks) => (
                        <strong className="font-semibold text-proph-ink">
                          {chunks}
                        </strong>
                      ),
                    })}
                  </dd>
                </div>
              </dl>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function YourRights({ t }: { t: PrivacyNoticeCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-6 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("rights.heading")}
        </h2>
        <p className="max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          {t.rich("rights.intro", {
            privacyLink: (chunks) => (
              <a href="/legal/privacy" className="text-proph-primary underline">
                {chunks}
              </a>
            ),
          })}
        </p>
        <ul className="mt-6 space-y-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          {rightKeys.map((key) => (
            <li key={key}>
              ·{" "}
              {t.rich(`rights.items.${key}`, {
                strong: (chunks) => (
                  <strong className="font-semibold text-proph-ink">
                    {chunks}
                  </strong>
                ),
              })}
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

export default async function PrivacyNoticePage() {
  const t = await getTranslations("kyc.privacyNotice");

  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <PrivacyMatrix t={t} />
      <YourRights t={t} />
    </>
  );
}
