import type { Metadata } from "next";
import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";

interface PageProps {
  searchParams: Promise<{ mode?: string | string[] }>;
}

type AuthCopy = Awaited<ReturnType<typeof getTranslations<"auth.signIn">>>;

const launchCountries = ["TW", "JP", "KR", "VN", "MY"] as const;
const postSignupStepKeys = ["age", "rg", "limit", "resources"] as const;

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("auth.signIn.metadata");

  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

export default async function AuthPage({ searchParams }: PageProps) {
  const [params, sectionFooterColumns, t] = await Promise.all([
    searchParams,
    getFooterColumns(["legal", "support"]),
    getTranslations("auth.signIn"),
  ]);
  const raw = Array.isArray(params.mode) ? params.mode[0] : params.mode;
  const mode: "signin" | "signup" = raw === "signup" ? "signup" : "signin";

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={
          <a
            href="/support"
            className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
          >
            {t("nav.supportCta")}
          </a>
        }
      />

      <ProphTile
        surface="light"
        kicker={t(`hero.${mode}.kicker`)}
        headline={t(`hero.${mode}.headline`)}
        tagline={t(`hero.${mode}.tagline`)}
      />

      <ModeToggle mode={mode} t={t} />

      {mode === "signup" ? <SignupForm t={t} /> : <SigninForm t={t} />}

      {mode === "signup" ? <PostSignupPreview t={t} /> : null}

      <ComplianceReminder t={t} />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

function ModeToggle({ mode, t }: { mode: "signin" | "signup"; t: AuthCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
        <div role="tablist" aria-label={t("modeToggle.ariaLabel")} className="flex gap-2">
          <a
            role="tab"
            aria-selected={mode === "signin"}
            href="/auth/sign-in"
            className={
              "inline-flex items-center rounded-proph-pill px-5 py-2 font-text text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors " +
              (mode === "signin"
                ? "bg-proph-primary text-proph-on-primary"
                : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary hover:text-proph-ink")
            }
          >
            {t("modeToggle.signin")}
          </a>
          <a
            role="tab"
            aria-selected={mode === "signup"}
            href="/auth/sign-in?mode=signup"
            className={
              "inline-flex items-center rounded-proph-pill px-5 py-2 font-text text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors " +
              (mode === "signup"
                ? "bg-proph-primary text-proph-on-primary"
                : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary hover:text-proph-ink")
            }
          >
            {t("modeToggle.signup")}
          </a>
        </div>
      </div>
    </section>
  );
}

function SigninForm({ t }: { t: AuthCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action="#" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              Email
            </span>
            <input
              type="email"
              name="email"
              autoComplete="username"
              placeholder="you@example.com"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="flex items-baseline justify-between">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t("signin.password")}
              </span>
              <a
                href="/support/contact?topic=password-reset"
                className="font-text text-[14px] tracking-[-0.224px] text-proph-primary underline"
              >
                {t("signin.forgotPassword")}
              </a>
            </span>
            <input
              type="password"
              name="password"
              autoComplete="current-password"
              placeholder="********"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="flex items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            <input type="checkbox" name="remember" className="h-4 w-4 accent-proph-primary" />
            {t("signin.remember")}
          </label>

          <div className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {t.rich("signin.mfaNotice", {
              strong: (chunks) => <strong className="font-semibold text-proph-ink">{chunks}</strong>,
              supportLink: (chunks) => (
                <a href="/support/contact" className="text-proph-primary underline">
                  {chunks}
                </a>
              ),
            })}
          </div>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("signin.submitNote")}
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {t("signin.submit")}
            </button>
          </div>

          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t.rich("signin.signupPrompt", {
              signupLink: (chunks) => (
                <a href="/auth/sign-in?mode=signup" className="text-proph-primary underline">
                  {chunks}
                </a>
              ),
            })}
          </p>
        </form>
      </div>
    </section>
  );
}

function SignupForm({ t }: { t: AuthCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action="#" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              Email
            </span>
            <input
              type="email"
              name="email"
              autoComplete="email"
              placeholder="you@example.com"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("signup.emailHint")}
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.birthday")}
            </span>
            <input
              type="date"
              name="birthday"
              required
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("signup.birthdayHint")}
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.country")}
            </span>
            <select
              name="country"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            >
              <option value="">{t("signup.countryPlaceholder")}</option>
              {launchCountries.map((code) => (
                <option key={code} value={code}>
                  {t(`countries.${code.toLowerCase()}`)}
                </option>
              ))}
            </select>
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("signup.countryHint")}
            </span>
          </label>

          <fieldset className="space-y-3">
            <legend className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.disclosures.legend")}
            </legend>

            <DisclosureCheckbox>
              {t.rich("signup.disclosures.terms", {
                termsLink: (chunks) => (
                  <a href="/legal/terms" className="text-proph-primary underline">
                    {chunks}
                  </a>
                ),
                privacyLink: (chunks) => (
                  <a href="/legal/privacy" className="text-proph-primary underline">
                    {chunks}
                  </a>
                ),
              })}
            </DisclosureCheckbox>

            <DisclosureCheckbox>
              {t.rich("signup.disclosures.points", {
                strong: (chunks) => <strong className="font-semibold text-proph-ink">{chunks}</strong>,
              })}
            </DisclosureCheckbox>

            <DisclosureCheckbox>
              {t.rich("signup.disclosures.entertainment", {
                strong: (chunks) => <strong className="font-semibold text-proph-ink">{chunks}</strong>,
              })}
            </DisclosureCheckbox>
          </fieldset>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("signup.submitNote")}
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {t("signup.submit")}
            </button>
          </div>

          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t.rich("signup.signinPrompt", {
              signinLink: (chunks) => (
                <a href="/auth/sign-in" className="text-proph-primary underline">
                  {chunks}
                </a>
              ),
            })}
          </p>
        </form>
      </div>
    </section>
  );
}

function DisclosureCheckbox({ children }: { children: ReactNode }) {
  return (
    <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
      <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary" />
      <span>{children}</span>
    </label>
  );
}

function PostSignupPreview({ t }: { t: AuthCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("postSignup.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("postSignup.intro")}
          </p>
        </header>

        <ol className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {postSignupStepKeys.map((key, index) => (
            <li
              key={key}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-primary">
                  {String(index + 1).padStart(2, "0")}
                </span>
                <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                  {t(`postSignup.steps.${key}.label`)}
                </h3>
              </div>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`postSignup.steps.${key}.body`)}
              </p>
            </li>
          ))}
        </ol>

        <p className="mt-6 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
          {t.rich("postSignup.next", {
            tier1Link: (chunks) => (
              <a href="/kyc/tier-1" className="text-proph-primary underline">
                {chunks}
              </a>
            ),
          })}
        </p>
      </div>
    </section>
  );
}

function ComplianceReminder({ t }: { t: AuthCopy }) {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
          {t("compliance.kicker")}
        </span>
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
          {t.rich("compliance.headline", {
            helplineLink: (chunks) => (
              <a href="tel:1925" className="text-proph-primary-on-dark hover:underline">
                {chunks}
              </a>
            ),
          })}
        </h2>
        <div className="mt-6">
          <ProphLinkButton variant="primary" href="/responsible-gaming/resources">
            {t("compliance.cta")}
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}
