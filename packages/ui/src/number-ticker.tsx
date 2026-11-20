"use client";

import { useEffect, useRef, useState } from "react";
import { cn } from "./cn";

export interface NumberTickerProps {
  value: number;
  durationMs?: number;
  decimals?: number;
  prefix?: string;
  suffix?: string;
  showSign?: boolean;
  className?: string;
}

export function NumberTicker({
  value,
  durationMs = 900,
  decimals = 0,
  prefix = "",
  suffix = "",
  showSign = false,
  className,
}: NumberTickerProps) {
  const [displayValue, setDisplayValue] = useState(value);
  const startValueRef = useRef(value);
  const startTimeRef = useRef<number | null>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const startVal = displayValue;
    const endVal = value;
    if (startVal === endVal) return;

    startValueRef.current = startVal;
    startTimeRef.current = null;

    const easeOutExpo = (x: number): number => {
      return x === 1 ? 1 : 1 - Math.pow(2, -10 * x);
    };

    const step = (timestamp: number) => {
      if (!startTimeRef.current) startTimeRef.current = timestamp;
      const elapsed = timestamp - startTimeRef.current;
      const progress = Math.min(elapsed / durationMs, 1);
      const eased = easeOutExpo(progress);

      const current = startVal + (endVal - startVal) * eased;
      setDisplayValue(current);

      if (progress < 1) {
        rafRef.current = requestAnimationFrame(step);
      } else {
        setDisplayValue(endVal);
      }
    };

    rafRef.current = requestAnimationFrame(step);

    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, [value, durationMs]);

  const sign = showSign && displayValue > 0 ? "+" : "";
  const formattedNumber = displayValue.toLocaleString("zh-Hant", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });

  return (
    <span
      className={cn(
        "inline-flex font-mono tabular-nums tracking-tight select-none",
        className,
      )}
    >
      {prefix}
      {sign}
      {formattedNumber}
      {suffix}
    </span>
  );
}
