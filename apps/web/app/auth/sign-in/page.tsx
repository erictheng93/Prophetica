import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { cookies } from "next/headers";
import { headers as requestHeaders } from "next/headers";
import { redirect } from "next/navigation";
import { ProphFooter, ProphSubNav, ProphTile } from "@prophetica/ui";
import { LanguageSwitcher } from "@/components/language-switcher";
import { authenticateWithPassword } from "@/lib/auth";
import { REFERRAL_ATTRIBUTION_COOKIE } from "@/lib/referral-attribution";
import { safeReturnTo } from "@/lib/safe-return-to";
import { setViewerSession } from "@/lib/session";
import { registerPublicAccount } from "@/lib/signup";
import { recordJourneyRedirect } from "@/lib/telemetry/journey-metrics";
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
    signup?: string | string[];
    return_to?: string | string[];
  }>;
}

type AuthCopy = Awaited<ReturnType<typeof getTranslations<"auth.signIn">>>;
type SigninStatusKey = "missing" | "invalid" | "unavailable";
type SignupStatusKey = "missing" | "invalid" | "conflict" | "unavailable";

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
  const signupStatus = signupStatusFromParam(params.signup);
  const rawReturnTo = Array.isArray(params.return_to)
    ? params.return_to[0]
    : params.return_to;
  const returnTo = safeReturnTo(rawReturnTo);

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
        glow="gold"
        gridPattern
        headline={t(`hero.${mode}.headline`)}
        tagline={t(`hero.${mode}.tagline`)}
      />

      <ModeToggle mode={mode} returnTo={returnTo} t={t} />

      {mode === "signup" ? (
        <SignupForm t={t} status={signupStatus} returnTo={returnTo} />
      ) : (
        <SigninForm t={t} status={signinStatus} returnTo={returnTo} />
      )}

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}

async function submitSigninAction(formData: FormData) {
  "use server";

  const identifier = readFormValue(formData, "identifier");
  const password = readFormValue(formData, "password");
  const returnTo = safeReturnTo(readFormValue(formData, "return_to"));
  const remember = formData.get("remember") === "on";

  if (!identifier.trim() || !password) {
    redirect(authPath({ login: "missing", returnTo }));
  }

  const result = await authenticateWithPassword(
    {
      identifier,
      password,
      remember,
    },
    {
      forwardedHeaders: await loginEventForwardHeaders(),
    },
  );

  if (result.status === "authenticated") {
    await setViewerSession({
      userId: result.userId,
      remember,
      upstreamSessionToken: result.sessionToken,
    });
    recordJourneyRedirect("/auth/sign-in", returnTo);
    redirect(returnTo);
  }

  if (
    result.status === "invalid_identifier" ||
    result.status === "invalid_credentials"
  ) {
    redirect(authPath({ login: "invalid", returnTo }));
  }

  redirect(authPath({ login: "unavailable", returnTo }));
}

async function loginEventForwardHeaders(): Promise<Headers> {
  const incoming = await requestHeaders();
  const forwarded = new Headers();
  for (const name of [
    "user-agent",
    "cf-connecting-ip",
    "cf-ipcountry",
    "cf-region",
    "cf-ipcity",
    "cf-asn",
    "x-forwarded-for",
    "x-real-ip",
    "x-request-id",
    "x-correlation-id",
  ]) {
    const value = incoming.get(name);
    if (value) {
      forwarded.set(name, value);
    }
  }
  return forwarded;
}

async function submitSignupAction(formData: FormData) {
  "use server";

  const email = readFormValue(formData, "email");
  const phone = readFormValue(formData, "phone");
  const password = readFormValue(formData, "password");
  const displayName = readFormValue(formData, "displayName");
  const returnTo = safeReturnTo(readFormValue(formData, "return_to"));
  const referralCode = (await cookies()).get(
    REFERRAL_ATTRIBUTION_COOKIE,
  )?.value;

  if (!email.trim() || !phone.trim() || !password || !displayName.trim()) {
    redirect(authPath({ mode: "signup", signup: "missing", returnTo }));
  }

  const result = await registerPublicAccount({
    email,
    phone,
    password,
    displayName,
    referralCode,
  });

  switch (result.status) {
    case "registered": {
      const nextPath = authPath({ signup: "registered", returnTo });
      recordJourneyRedirect("/auth/sign-in", nextPath);
      redirect(nextPath);
    }
    case "invalid":
      redirect(authPath({ mode: "signup", signup: "invalid", returnTo }));
    case "conflict":
      redirect(authPath({ mode: "signup", signup: "conflict", returnTo }));
    case "unavailable":
      redirect(authPath({ mode: "signup", signup: "unavailable", returnTo }));
  }
}
function ModeToggle({
  mode,
  returnTo,
  t,
}: {
  mode: "signin" | "signup";
  returnTo: string;
  t: AuthCopy;
}) {
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
            href={authPath({ returnTo })}
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
            href={authPath({ mode: "signup", returnTo })}
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

function SigninForm({
  t,
  status,
  returnTo,
}: {
  t: AuthCopy;
  status?: SigninStatusKey;
  returnTo: string;
}) {
  const statusBannerId = status ? `signin-status-${status}` : undefined;
  const passwordDescribedBy = status === "invalid" ? statusBannerId : undefined;

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form action={submitSigninAction} className="space-y-6">
          <input type="hidden" name="return_to" value={returnTo} />
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
                  href={authPath({ mode: "signup", returnTo })}
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

function SignupForm({
  t,
  status,
  returnTo,
}: {
  t: AuthCopy;
  status?: SignupStatusKey;
  returnTo: string;
}) {
  const statusBannerId = status ? `signup-status-${status}` : undefined;

  return (
    <section className="bg-proph-canvas" aria-labelledby="signup-heading">
      <div className="mx-auto max-w-2xl px-6 py-12 md:px-10">
        <form
          action={submitSignupAction}
          className="space-y-6 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 md:p-8"
        >
          <input type="hidden" name="return_to" value={returnTo} />
          <p className="font-text text-[12px] uppercase leading-none tracking-[0] text-proph-primary">
            {t("signup.form.eyebrow")}
          </p>
          <h2
            id="signup-heading"
            className="mt-4 font-display text-[28px] font-semibold leading-[1.1] tracking-[0] text-proph-ink sm:text-[34px]"
          >
            {t("signup.form.heading")}
          </h2>
          <p className="mt-4 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("signup.form.body")}
          </p>

          {status && statusBannerId ? (
            <SignupStatusBanner id={statusBannerId} t={t} status={status} />
          ) : null}

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.form.displayName")}
            </span>
            <input
              type="text"
              name="displayName"
              autoComplete="name"
              required
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.form.email")}
            </span>
            <input
              type="email"
              name="email"
              autoComplete="email"
              required
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.form.phone")}
            </span>
            <input
              type="tel"
              name="phone"
              autoComplete="tel"
              inputMode="tel"
              required
              aria-describedby="signup-phone-hint"
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span
              id="signup-phone-hint"
              className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48"
            >
              {t("signup.form.phoneHint")}
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("signup.form.password")}
            </span>
            <input
              type="password"
              name="password"
              autoComplete="new-password"
              minLength={8}
              required
              className="mt-1 w-full min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <div className="mt-6 flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 sm:flex-row sm:items-center">
            <button
              type="submit"
              className="inline-flex min-h-11 select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary transition-colors hover:bg-proph-primary-focus"
            >
              {t("signup.form.submit")}
            </button>
            <a
              href={authPath({ returnTo })}
              className="inline-flex min-h-11 select-none items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink"
            >
              {t("signup.form.signinCta")}
            </a>
          </div>
        </form>
      </div>
    </section>
  );
}

function SignupStatusBanner({
  id,
  t,
  status,
}: {
  id: string;
  t: AuthCopy;
  status: SignupStatusKey;
}) {
  return (
    <p
      id={id}
      role="alert"
      aria-live="polite"
      className="rounded-proph-md border border-proph-feedback-error/40 bg-proph-feedback-error/10 px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink"
    >
      {t(`signup.status.${status}`)}
    </p>
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
    default:
      return undefined;
  }
}

function signupStatusFromParam(
  value: string | string[] | undefined,
): SignupStatusKey | undefined {
  const status = Array.isArray(value) ? value[0] : value;

  switch (status) {
    case "missing":
    case "invalid":
    case "conflict":
    case "unavailable":
      return status;
    default:
      return undefined;
  }
}

function readFormValue(formData: FormData, key: string): string {
  const value = formData.get(key);
  return typeof value === "string" ? value : "";
}

function authPath({
  mode,
  login,
  signup,
  returnTo,
}: {
  mode?: "signup";
  login?: SigninStatusKey;
  signup?: SignupStatusKey | "registered";
  returnTo?: string;
}): string {
  const params = new URLSearchParams();
  if (mode) {
    params.set("mode", mode);
  }
  if (login) {
    params.set("login", login);
  }
  if (signup) {
    params.set("signup", signup);
  }
  const safe = safeReturnTo(returnTo);
  if (safe !== "/portfolio") {
    params.set("return_to", safe);
  }
  const query = params.toString();
  return query ? `/auth/sign-in?${query}` : "/auth/sign-in";
}
