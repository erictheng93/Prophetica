import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { redirect } from "next/navigation";
import { ProphFooter, ProphSubNav, ProphTile } from "@prophetica/ui";
import { LanguageSwitcher } from "@/components/language-switcher";
import { authenticateWithPassword } from "@/lib/auth";
import { setViewerSession } from "@/lib/session";
import {
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
  SiteThemeToggle,
} from "@/lib/site-nav";

interface PageProps {
  searchParams: Promise<{
    mode?: string | string[];
    login?: string | string[];
  }>;
}

type AuthCopy = Awaited<ReturnType<typeof getTranslations<"auth.signIn">>>;
type SigninStatusKey = "missing" | "invalid" | "unavailable" | "mfaRequired";

const officialLineHref =
  process.env.NEXT_PUBLIC_OFFICIAL_LINE_URL ||
  "/support/contact?topic=beta-access";
const officialLineIsExternal = officialLineHref.startsWith("http");

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
  const signinStatus = signinStatusFromParam(params.login);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={
          <div className="flex items-center gap-2 sm:gap-3">
            <LanguageSwitcher />
            <a
              href="/support"
              className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
            >
              {t("nav.supportCta")}
            </a>
            <SiteThemeToggle />
          </div>
        }
      />

      <ProphTile
        surface="light"
        headline={t(`hero.${mode}.headline`)}
        tagline={t(`hero.${mode}.tagline`)}
      />

      <ModeToggle mode={mode} t={t} />

      {mode === "signup" ? (
        <SignupForm t={t} />
      ) : (
        <SigninForm t={t} status={signinStatus} />
      )}

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

async function submitSigninAction(formData: FormData) {
  "use server";

  const identifier = readFormValue(formData, "identifier");
  const password = readFormValue(formData, "password");
  const mfaCode = readFormValue(formData, "mfaCode");
  const mfaRecoveryCode = readFormValue(formData, "mfaRecoveryCode");
  const remember = formData.get("remember") === "on";

  if (!identifier.trim() || !password) {
    redirect("/auth/sign-in?login=missing");
  }

  const result = await authenticateWithPassword({
    identifier,
    password,
    remember,
    mfaCode,
    mfaRecoveryCode,
  });

  if (result.status === "authenticated") {
    await setViewerSession({ userId: result.userId, remember });
    redirect("/portfolio");
  }

  if (result.status === "mfa_required") {
    redirect("/auth/sign-in?login=mfa-required");
  }

  if (
    result.status === "invalid_identifier" ||
    result.status === "invalid_credentials"
  ) {
    redirect("/auth/sign-in?login=invalid");
  }

  redirect("/auth/sign-in?login=unavailable");
}
function ModeToggle({ mode, t }: { mode: "signin" | "signup"; t: AuthCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-6 md:px-10">
        <div
          role="tablist"
          aria-label={t("modeToggle.ariaLabel")}
          className="flex gap-2"
        >
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

function SigninForm({ t, status }: { t: AuthCopy; status?: SigninStatusKey }) {
  const statusBannerId = status ? `signin-status-${status}` : undefined;
  const passwordDescribedBy = status === "invalid" ? statusBannerId : undefined;

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action={submitSigninAction} className="space-y-6">
          {status && statusBannerId ? (
            <SigninStatusBanner id={statusBannerId} t={t} status={status} />
          ) : null}

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signin.identifier.label")}
            </span>
            <input
              id="signin-identifier"
              type="text"
              name="identifier"
              autoComplete="username"
              autoCapitalize="none"
              enterKeyHint="next"
              spellCheck={false}
              required
              aria-describedby="signin-identifier-hint"
              placeholder={t("signin.identifier.placeholder")}
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span
              id="signin-identifier-hint"
              className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48"
            >
              {t("signin.identifier.hint")}
            </span>
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
              required
              aria-invalid={status === "invalid" ? true : undefined}
              aria-describedby={passwordDescribedBy}
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <fieldset className="space-y-3 rounded-proph-md border border-proph-hairline bg-proph-canvas-parchment/50 p-4">
            <legend className="px-1 font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signin.mfa.legend")}
            </legend>
            <p className="font-text text-[12px] leading-[1.33] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("signin.mfa.hint")}
            </p>
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t("signin.mfa.codeLabel")}
              </span>
              <input
                type="text"
                name="mfaCode"
                inputMode="numeric"
                autoComplete="one-time-code"
                pattern="[0-9]{6}"
                maxLength={6}
                placeholder={t("signin.mfa.codePlaceholder")}
                className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t("signin.mfa.recoveryLabel")}
              </span>
              <input
                type="text"
                name="mfaRecoveryCode"
                autoComplete="off"
                placeholder={t("signin.mfa.recoveryPlaceholder")}
                className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
          </fieldset>

          <label className="flex min-h-11 items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            <input
              type="checkbox"
              name="remember"
              className="h-4 w-4 accent-proph-primary"
            />
            {t("signin.remember")}
          </label>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <button
              type="submit"
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary transition-colors hover:bg-proph-primary-focus"
            >
              {t("signin.submit")}
            </button>
          </div>

          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t.rich("signin.signupPrompt", {
              signupLink: (chunks) => (
                <a
                  href="/auth/sign-in?mode=signup"
                  className="text-proph-primary underline"
                >
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

function SigninStatusBanner({
  id,
  t,
  status,
}: {
  id: string;
  t: AuthCopy;
  status: SigninStatusKey;
}) {
  return (
    <p
      id={id}
      role="alert"
      aria-live="polite"
      className="rounded-proph-md border border-proph-feedback-error/40 bg-proph-feedback-error/10 px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink"
    >
      {t(`signin.status.${status}`)}
    </p>
  );
}

function SignupForm({ t }: { t: AuthCopy }) {
  return (
    <section
      className="bg-proph-canvas"
      aria-labelledby="signup-beta-gate-heading"
    >
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <div className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 md:p-8">
          <p className="font-text text-[12px] uppercase leading-none tracking-[0] text-proph-primary">
            {t("signup.betaGate.eyebrow")}
          </p>
          <h2
            id="signup-beta-gate-heading"
            className="mt-4 font-display text-[28px] font-semibold leading-[1.1] tracking-[0] text-proph-ink sm:text-[34px]"
          >
            {t("signup.betaGate.heading")}
          </h2>
          <p className="mt-4 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("signup.betaGate.body")}
          </p>
          <div className="mt-6 flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 sm:flex-row sm:items-center">
            <a
              href={officialLineHref}
              target={officialLineIsExternal ? "_blank" : undefined}
              rel={officialLineIsExternal ? "noreferrer" : undefined}
              className="inline-flex min-h-11 select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary transition-colors hover:bg-proph-primary-focus"
            >
              {t("signup.betaGate.lineCta")}
            </a>
            <a
              href="/auth/sign-in"
              className="inline-flex min-h-11 select-none items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink"
            >
              {t("signup.betaGate.signinCta")}
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

function signinStatusFromParam(
  value: string | string[] | undefined,
): SigninStatusKey | undefined {
  const status = Array.isArray(value) ? value[0] : value;

  switch (status) {
    case "missing":
    case "invalid":
    case "unavailable":
      return status;
    case "mfa-required":
      return "mfaRequired";
    default:
      return undefined;
  }
}

function readFormValue(formData: FormData, key: string): string {
  const value = formData.get(key);
  return typeof value === "string" ? value : "";
}
