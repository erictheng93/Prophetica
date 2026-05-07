"use client";

import { useState } from "react";

interface CopyButtonProps {
  value: string;
  label: string;
  copiedLabel: string;
  ariaLabel: string;
  variant?: "primary" | "outline";
}

// SPEC-011 v0.2.1 §5.1: copy share-link / referral-code button.
// Server-rendered dashboard is otherwise pure-RSC; this is the only
// interactive surface needed.
export function CopyButton({
  value,
  label,
  copiedLabel,
  ariaLabel,
  variant = "outline",
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  async function onClick() {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API unavailable (older browser / insecure context).
      // Silently no-op — the visible code is already user-selectable.
    }
  }

  const baseClass =
    "inline-flex items-center justify-center rounded-proph-pill px-4 py-1.5 font-text text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors";
  const variantClass =
    variant === "primary"
      ? "bg-proph-primary text-proph-on-primary hover:bg-proph-primary-strong disabled:opacity-50"
      : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary hover:text-proph-ink disabled:opacity-50";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!value}
      aria-label={ariaLabel}
      className={`${baseClass} ${variantClass}`}
    >
      {copied ? copiedLabel : label}
    </button>
  );
}
