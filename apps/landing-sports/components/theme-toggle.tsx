"use client";

import { useEffect, useState } from "react";
import { Moon, Sun } from "@phosphor-icons/react";
import { cn } from "@prophetica/ui";

export type ThemeMode = "light" | "dark";

export interface ThemeToggleLabels {
  dark: string;
  light: string;
  switchToDark: string;
  switchToLight: string;
}

export interface ThemeToggleProps {
  labels: ThemeToggleLabels;
}

const storageKey = "prophetica-theme";

function isThemeMode(value: string | null | undefined): value is ThemeMode {
  return value === "light" || value === "dark";
}

function applyTheme(theme: ThemeMode) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

function readStoredTheme(): ThemeMode {
  try {
    const stored = window.localStorage.getItem(storageKey);
    return isThemeMode(stored) ? stored : "dark";
  } catch {
    return "dark";
  }
}

export function ThemeToggle({ labels }: ThemeToggleProps) {
  const [theme, setTheme] = useState<ThemeMode>("dark");
  const isDark = theme === "dark";
  const label = isDark ? labels.switchToLight : labels.switchToDark;

  useEffect(() => {
    const documentTheme = document.documentElement.dataset.theme;
    const initialTheme = isThemeMode(documentTheme)
      ? documentTheme
      : readStoredTheme();

    applyTheme(initialTheme);
    setTheme(initialTheme);
  }, []);

  function handleToggle() {
    setTheme((currentTheme) => {
      const updatedTheme: ThemeMode =
        currentTheme === "dark" ? "light" : "dark";

      applyTheme(updatedTheme);

      try {
        window.localStorage.setItem(storageKey, updatedTheme);
      } catch {
        // Keep the visible theme change even when storage is unavailable.
      }

      return updatedTheme;
    });
  }

  return (
    <button
      type="button"
      role="switch"
      aria-checked={isDark}
      aria-label={label}
      title={label}
      onClick={handleToggle}
      className={cn(
        "relative inline-flex h-9 w-9 shrink-0 items-center rounded-proph-pill border border-[#D4A857]/35 bg-[var(--sports-control-bg)] p-1 text-[var(--sports-text)] sm:w-[68px]",
        "transition-colors hover:border-[#D4A857] active:scale-[0.95]",
        "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#F0D78C]",
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "absolute left-1 top-1 h-7 w-7 rounded-proph-full bg-[#D4A857] transition-transform duration-150 ease-out",
          isDark ? "translate-x-0 sm:translate-x-8" : "translate-x-0",
        )}
      />
      <span className="relative z-10 flex w-full items-center justify-center sm:justify-between sm:px-1">
        <Sun
          aria-hidden="true"
          weight={isDark ? "regular" : "fill"}
          className={cn(
            "h-4 w-4 transition-colors",
            isDark ? "hidden text-[#8A8378] sm:block" : "text-[#050303]",
          )}
        />
        <Moon
          aria-hidden="true"
          weight={isDark ? "fill" : "regular"}
          className={cn(
            "h-4 w-4 transition-colors",
            isDark ? "text-[#050303]" : "hidden text-[#8A8378] sm:block",
          )}
        />
      </span>
      <span className="sr-only">{isDark ? labels.dark : labels.light}</span>
    </button>
  );
}
