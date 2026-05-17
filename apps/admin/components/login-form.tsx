"use client";

import { useState } from "react";
import { useSearchParams } from "next/navigation";
import { adminCopy } from "@/lib/admin-copy";
import { resolveLoginRedirect } from "@/lib/login-redirect";

type Feedback = {
  tone: "error" | "success";
  text: string;
};

export function LoginForm() {
  const params = useSearchParams();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const intendedRedirect = params?.get("from") ?? null;

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    let navigating = false;
    setFeedback(null);
    setSubmitting(true);
    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ email, password }),
      });
      if (res.ok) {
        const data = (await res.json()) as { redirect: string };
        const redirect = resolveLoginRedirect(intendedRedirect, data.redirect);
        navigating = true;
        setFeedback({ tone: "success", text: adminCopy.login.successRedirect });
        window.location.assign(redirect);
        return;
      }
      if (res.status === 401 || res.status === 400) {
        setFeedback({ tone: "error", text: adminCopy.login.invalid });
      } else {
        setFeedback({ tone: "error", text: adminCopy.login.unknownError });
      }
    } catch {
      setFeedback({ tone: "error", text: adminCopy.login.unknownError });
    } finally {
      if (!navigating) {
        setSubmitting(false);
      }
    }
  }

  return (
    <form className="space-y-5" onSubmit={handleSubmit}>
      <div className="space-y-2">
        <label
          className="block text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]"
          htmlFor="login-email"
        >
          {adminCopy.login.emailLabel}
        </label>
        <input
          id="login-email"
          type="email"
          autoComplete="email"
          required
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          className="w-full rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink focus:border-proph-primary focus:outline-none focus:ring-2 focus:ring-proph-primary-focus"
        />
      </div>
      <div className="space-y-2">
        <label
          className="block text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]"
          htmlFor="login-password"
        >
          {adminCopy.login.passwordLabel}
        </label>
        <input
          id="login-password"
          type="password"
          autoComplete="current-password"
          required
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          className="w-full rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink focus:border-proph-primary focus:outline-none focus:ring-2 focus:ring-proph-primary-focus"
        />
      </div>
      {feedback && (
        <p
          aria-live="polite"
          className={[
            "rounded-proph-sm border bg-proph-canvas px-3 py-2 text-[14px] leading-[1.43] tracking-[-0.224px]",
            feedback.tone === "error"
              ? "border-proph-feedback-error text-proph-feedback-error"
              : "border-proph-primary text-proph-primary",
          ].join(" ")}
        >
          {feedback.text}
        </p>
      )}
      <button
        type="submit"
        disabled={submitting}
        className="inline-flex h-12 w-full items-center justify-center rounded-proph-pill bg-proph-primary px-6 text-[17px] font-normal leading-[1.47] tracking-[-0.374px] text-proph-on-primary transition-transform duration-150 ease-out active:scale-[0.97] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-proph-primary-focus disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? adminCopy.login.submitting : adminCopy.login.submit}
      </button>
    </form>
  );
}
