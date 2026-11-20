"use client";

import { useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import type {
  CooldownDuration,
  CooldownStatus,
} from "@/lib/responsible-gaming-api";

interface CooldownOptionCopy {
  id: CooldownDuration;
  label: string;
  body: string;
}

interface CooldownFormCopy {
  heading: string;
  intro: string;
  legend: string;
  statusLabel: string;
  statusActive: string;
  statusInactive: string;
  statusFallback: string;
  consent: string;
  submit: string;
  submitPending: string;
  success: string;
  genericError: string;
}

interface CooldownActivationFormProps {
  callout: ReactNode;
  copy: CooldownFormCopy;
  initialStatus: CooldownStatus;
  options: CooldownOptionCopy[];
}

export function CooldownActivationForm({
  callout,
  copy,
  initialStatus,
  options,
}: CooldownActivationFormProps) {
  const [status, setStatus] = useState(initialStatus);
  const [selectedDuration, setSelectedDuration] = useState<CooldownDuration>(
    initialStatus.availableDurations[0] ?? "24h",
  );
  const [consent, setConsent] = useState(false);
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const activeEnd = useMemo(
    () => formatEndDate(status.endsAtEpochSeconds),
    [status.endsAtEpochSeconds],
  );
  const statusText = status.active
    ? copy.statusActive.replace("{date}", activeEnd)
    : copy.statusInactive;
  const formLocked = pending || status.active;

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (formLocked || !consent) return;

    const formData = new FormData(event.currentTarget);
    const submittedDuration = formData.get("duration");
    const duration = isCooldownDuration(submittedDuration)
      ? submittedDuration
      : selectedDuration;

    setPending(true);
    setNotice(null);
    setError(null);

    try {
      const response = await fetch("/api/responsible-gaming/cooldown", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ duration, consent }),
      });
      const payload = (await response.json()) as unknown;
      if (!response.ok) {
        throw new Error(
          isErrorPayload(payload) && typeof payload.message === "string"
            ? payload.message
            : copy.genericError,
        );
      }

      setStatus(payload as CooldownStatus);
      setConsent(false);
      setNotice(copy.success);
    } catch (submitError) {
      setError(
        submitError instanceof Error ? submitError.message : copy.genericError,
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {copy.heading}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {copy.intro}
          </p>
        </header>

        <div className="mb-6 rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas-parchment p-6 shadow-sm">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {copy.statusLabel}
          </div>
          <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink">
            {statusText}
          </p>
          {status.source === "fallback:api-gateway-unavailable" ? (
            <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {copy.statusFallback}
            </p>
          ) : null}
        </div>

        <form onSubmit={onSubmit}>
          <fieldset className="space-y-6" disabled={pending}>
            <fieldset className="grid grid-cols-1 gap-4 md:grid-cols-3">
              <legend className="sr-only">{copy.legend}</legend>
              {options.map((option) => (
                <label
                  key={option.id}
                  className="flex cursor-pointer flex-col gap-3 rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-sm transition-all duration-200 hover:-translate-y-0.5 has-[:checked]:border-proph-primary has-[:checked]:bg-proph-canvas-parchment has-[:checked]:shadow-proph-card"
                >
                  <span className="flex items-center gap-3">
                    <input
                      type="radio"
                      name="duration"
                      value={option.id}
                      checked={selectedDuration === option.id}
                      disabled={formLocked}
                      onChange={() => setSelectedDuration(option.id)}
                      className="h-4 w-4 accent-proph-primary"
                    />
                    <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                      {option.label}
                    </span>
                  </span>
                  <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {option.body}
                  </span>
                </label>
              ))}
            </fieldset>

            <div className="rounded-proph-xl border border-proph-primary/30 bg-proph-canvas-parchment p-6">
              <p className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
                {callout}
              </p>
            </div>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
              <label className="flex min-h-11 items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                <input
                  type="checkbox"
                  required
                  checked={consent}
                  disabled={formLocked}
                  onChange={(event) => setConsent(event.target.checked)}
                  className="h-4 w-4 accent-proph-primary"
                />
                {copy.consent}
              </label>
              <button
                type="submit"
                disabled={formLocked || !consent}
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                {pending ? copy.submitPending : copy.submit}
              </button>
            </div>
          </fieldset>
        </form>

        {notice ? (
          <p className="mt-4 rounded-proph-md border border-proph-primary/30 bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-primary">
            {notice}
          </p>
        ) : null}
        {error ? (
          <p className="mt-4 rounded-proph-md border border-red-200 bg-red-50 px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-red-700">
            {error}
          </p>
        ) : null}
      </div>
    </section>
  );
}

function formatEndDate(epochSeconds: number | null): string {
  if (epochSeconds === null) return "-";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(epochSeconds * 1000));
}

function isErrorPayload(value: unknown): value is { message?: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isCooldownDuration(
  value: FormDataEntryValue | null,
): value is CooldownDuration {
  return value === "24h" || value === "7d" || value === "30d";
}
