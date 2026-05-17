"use client";

import { useEffect, useState } from "react";
import { adminCopy } from "@/lib/admin-copy";
import {
  assertPasskey,
  browserSupportsPasskeys,
  registerPasskey,
} from "@/lib/passkey-client";
import { StatusPill } from "./status-pill";

type Feedback = {
  tone: "error" | "success";
  text: string;
};

export function PasskeySecurityPanel() {
  const [supported, setSupported] = useState<boolean | null>(null);
  const [busyAction, setBusyAction] = useState<"register" | "assert" | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  useEffect(() => {
    setSupported(browserSupportsPasskeys());
  }, []);

  async function runPasskeyAction(action: "register" | "assert") {
    if (!supported) {
      setFeedback({ tone: "error", text: adminCopy.passkeys.unsupported });
      return;
    }
    setBusyAction(action);
    setFeedback(null);
    try {
      if (action === "register") {
        await registerPasskey();
        setFeedback({ tone: "success", text: adminCopy.passkeys.registered });
      } else {
        await assertPasskey();
        setFeedback({ tone: "success", text: adminCopy.passkeys.asserted });
      }
    } catch {
      setFeedback({ tone: "error", text: adminCopy.passkeys.failed });
    } finally {
      setBusyAction(null);
    }
  }

  return (
    <section
      aria-labelledby="passkey-security-title"
      className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
    >
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <StatusPill tone={supported === false ? "warning" : "success"}>
              {supported === false ? adminCopy.passkeys.statusUnavailable : adminCopy.passkeys.statusReady}
            </StatusPill>
            <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              WebAuthn / passkey
            </span>
          </div>
          <h2
            id="passkey-security-title"
            className="mt-3 text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
          >
            {adminCopy.passkeys.title}
          </h2>
          <p className="mt-2 max-w-3xl text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            {adminCopy.passkeys.description}
          </p>
        </div>
      </div>
      <div className="mt-5 flex flex-col gap-3 sm:flex-row">
        <button
          type="button"
          disabled={busyAction !== null || supported !== true}
          onClick={() => runPasskeyAction("register")}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-on-primary transition-transform duration-150 ease-out active:scale-[0.97] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-proph-primary-focus disabled:cursor-not-allowed disabled:opacity-60"
        >
          {busyAction === "register" ? adminCopy.passkeys.registering : adminCopy.passkeys.registerCta}
        </button>
        <button
          type="button"
          disabled={busyAction !== null || supported !== true}
          onClick={() => runPasskeyAction("assert")}
          className="inline-flex h-11 items-center justify-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-5 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink transition-transform duration-150 ease-out active:scale-[0.97] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-proph-primary-focus disabled:cursor-not-allowed disabled:opacity-60"
        >
          {busyAction === "assert" ? adminCopy.passkeys.asserting : adminCopy.passkeys.assertCta}
        </button>
      </div>
      {feedback && (
        <p
          aria-live="polite"
          className={[
            "mt-4 rounded-proph-sm border bg-proph-canvas px-3 py-2 text-[14px] leading-[1.43] tracking-[-0.224px]",
            feedback.tone === "error"
              ? "border-proph-feedback-error text-proph-feedback-error"
              : "border-proph-primary text-proph-primary",
          ].join(" ")}
        >
          {feedback.text}
        </p>
      )}
    </section>
  );
}
