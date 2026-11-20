import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { redirect } from "next/navigation";
import { ProphTile } from "@prophetica/ui";
import {
  resolveKycSubmitErrorStatus,
  submitTierOneVerification,
  type KycSubmitErrorStatus,
} from "@/lib/kyc-api";
import { safeReturnTo } from "@/lib/safe-return-to";
import { getViewer } from "@/lib/session";
import { recordJourneyRedirect } from "@/lib/telemetry/journey-metrics";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.tier1.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

// SPEC-005 §2.4 — first-launch 5 countries with verification-method differences.
// PH (PhilSys / passport) is intentionally weaker on Tier 1; failures route to Tier 2.
interface CountrySpec {
  code: string;
  flag: string;
  key: "tw" | "jp" | "kr" | "vn" | "my";
}

const countries: CountrySpec[] = [
  { code: "TW", flag: "🇹🇼", key: "tw" },
  { code: "JP", flag: "🇯🇵", key: "jp" },
  { code: "KR", flag: "🇰🇷", key: "kr" },
  { code: "VN", flag: "🇻🇳", key: "vn" },
  { code: "MY", flag: "🇲🇾", key: "my" },
];

type Tier1Copy = Awaited<ReturnType<typeof getTranslations<"kyc.tier1">>>;

interface PageProps {
  searchParams?: Promise<{
    return_to?: string | string[];
    submission_error?: string | string[];
  }>;
}

async function submitTierOneAction(formData: FormData) {
  "use server";

  const viewer = await getViewer();
  if (!viewer?.userId) {
    redirect("/auth/sign-in");
  }

  const returnTo = safeReturnTo(
    readFormValue(formData, "return_to"),
    "/kyc/status",
  );
  try {
    await submitTierOneVerification({
      userId: viewer.userId,
      legalName: readFormValue(formData, "legalName"),
      birthday: readFormValue(formData, "birthday"),
      country: readFormValue(formData, "country"),
      documentId: readFormValue(formData, "documentId"),
      consent: formData.get("consent") === "on",
    });
  } catch (error) {
    redirect(tierOneFailurePath(returnTo, resolveKycSubmitErrorStatus(error)));
  }
  recordJourneyRedirect("/kyc/tier-1", returnTo);
  redirect(returnTo);
}

function PrivacyReminder({ t }: { t: Tier1Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-6">
          <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
            {t("privacy.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t.rich("privacy.body", {
              privacyLink: (chunks) => (
                <a
                  href="/kyc/privacy-notice"
                  className="text-proph-primary underline"
                >
                  {chunks}
                </a>
              ),
              strong: (chunks) => (
                <strong className="font-semibold text-proph-ink">
                  {chunks}
                </strong>
              ),
            })}
          </p>
        </article>
      </div>
    </section>
  );
}

function CountryGuide({ t }: { t: Tier1Copy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("countries.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("countries.intro")}
          </p>
        </header>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {countries.map((c) => (
            <li
              key={c.code}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline gap-3">
                <span aria-hidden className="text-[24px]">
                  {c.flag}
                </span>
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`countries.items.${c.key}.name`)}
                </h3>
              </div>
              <div className="mt-3 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t("countries.documentLabel")}
              </div>
              <p className="font-text text-[17px] tracking-[-0.374px] text-proph-ink">
                {t(`countries.items.${c.key}.document`)}
              </p>
              <div className="mt-2 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t("countries.formatLabel")}
              </div>
              <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`countries.items.${c.key}.format`)}
              </p>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`countries.items.${c.key}.notes`)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function Tier1Form({
  t,
  returnTo,
  submissionError,
}: {
  t: Tier1Copy;
  returnTo: string;
  submissionError: KycSubmitErrorStatus | null;
}) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("form.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t.rich("form.intro", {
              strong: (chunks) => (
                <strong className="font-semibold text-proph-ink">
                  {chunks}
                </strong>
              ),
            })}
          </p>
        </header>

        <form action={submitTierOneAction}>
          <input type="hidden" name="return_to" value={returnTo} />
          {submissionError ? (
            <div
              role="alert"
              className="mb-6 rounded-proph-md border border-proph-feedback-error/40 bg-proph-feedback-error/10 px-4 py-3"
            >
              <p className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t(`form.errors.${submissionError}.title`)}
              </p>
              <p className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`form.errors.${submissionError}.body`)}
              </p>
            </div>
          ) : null}
          <fieldset className="space-y-6">
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <label className="flex flex-col gap-2">
                <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                  {t("form.fields.legalName")}
                </span>
                <input
                  type="text"
                  name="legalName"
                  placeholder={t("form.fields.legalNamePlaceholder")}
                  className="min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                />
              </label>

              <label className="flex flex-col gap-2">
                <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                  {t("form.fields.birthday")}
                </span>
                <input
                  type="date"
                  name="birthday"
                  className="min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
                />
              </label>

              <label className="flex flex-col gap-2">
                <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                  {t("form.fields.country")}
                </span>
                <select
                  name="country"
                  className="min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
                >
                  <option value="">
                    {t("form.fields.countryPlaceholder")}
                  </option>
                  {countries.map((c) => (
                    <option key={c.code} value={c.code}>
                      {t("form.fields.countryOption", {
                        country: t(`countries.items.${c.key}.name`),
                        document: t(`countries.items.${c.key}.document`),
                      })}
                    </option>
                  ))}
                </select>
              </label>

              <label className="flex flex-col gap-2 md:col-span-1">
                <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                  {t("form.fields.documentId")}
                </span>
                <input
                  type="text"
                  name="documentId"
                  inputMode="text"
                  autoComplete="off"
                  spellCheck={false}
                  placeholder={t("form.fields.documentIdPlaceholder")}
                  className="min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                />
              </label>
            </div>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              <input
                type="checkbox"
                name="consent"
                required
                className="mt-1 h-4 w-4 accent-proph-primary"
              />
              <span>
                {t.rich("form.consent", {
                  privacyLink: (chunks) => (
                    <a
                      href="/kyc/privacy-notice"
                      className="text-proph-primary underline"
                    >
                      {chunks}
                    </a>
                  ),
                  termsLink: (chunks) => (
                    <a
                      href="/legal/terms"
                      className="text-proph-primary underline"
                    >
                      {chunks}
                    </a>
                  ),
                })}
              </span>
            </label>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
              <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
                {t("form.footnote")}
              </p>
              <button
                type="submit"
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
              >
                {t("form.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

export default async function Tier1Page({ searchParams }: PageProps) {
  const viewer = await getViewer();
  if (!viewer?.userId) {
    redirect("/auth/sign-in");
  }

  const t = await getTranslations("kyc.tier1");
  const params = await searchParams;
  const returnTo = safeReturnTo(
    firstQueryValue(params?.return_to),
    "/kyc/status",
  );
  const submissionError = parseKycSubmitErrorStatus(
    firstQueryValue(params?.submission_error),
  );

  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <PrivacyReminder t={t} />
      <CountryGuide t={t} />
      <Tier1Form t={t} returnTo={returnTo} submissionError={submissionError} />
    </>
  );
}

function readFormValue(formData: FormData, key: string): string {
  const value = formData.get(key);
  return typeof value === "string" ? value : "";
}

function firstQueryValue(value: string | string[] | undefined): string {
  return Array.isArray(value) ? (value[0] ?? "") : (value ?? "");
}

function parseKycSubmitErrorStatus(value: string): KycSubmitErrorStatus | null {
  switch (value) {
    case "service_unavailable":
    case "duplicate_document":
    case "invalid_submission":
      return value;
    default:
      return null;
  }
}

function tierOneFailurePath(
  returnTo: string,
  submissionError: KycSubmitErrorStatus,
): string {
  const params = new URLSearchParams({ submission_error: submissionError });
  if (returnTo !== "/kyc/status") {
    params.set("return_to", returnTo);
  }
  return `/kyc/tier-1?${params.toString()}`;
}
