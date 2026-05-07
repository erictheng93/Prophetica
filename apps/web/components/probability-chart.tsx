"use client";

import { useEffect, useRef } from "react";
import {
  AreaSeries,
  ColorType,
  createChart,
  type IChartApi,
  type ISeriesApi,
  LineStyle,
} from "lightweight-charts";
import { colors, fontFamilies } from "@prophetica/design-tokens";
import type { ProbabilityPoint } from "@/lib/mock-markets";

interface Props {
  data: ProbabilityPoint[];
  /** Height in px; the chart fills container width responsively. */
  height?: number;
  ariaLabel: string;
}

/*
 * YES-price-over-time chart powered by Lightweight Charts (TradingView, MIT).
 *
 * Why a client component
 *   createChart binds to a DOM ref and only runs in the browser. Lightweight
 *   Charts has no SSR support. SEO impact is acceptable: the searchable
 *   content on /markets/[id] (title, brief, current YES/NO probability bar)
 *   is fully SSR'd; the time-series chart is supplementary. Decision logged
 *   2026-05-06 — keep Lightweight Charts; revisit only if a Core Web Vital
 *   regression or a hard SEO-snippet requirement appears.
 *
 * Theme tokens
 *   Lightweight Charts options accept JS string literals, not CSS variables.
 *   We import resolved color values from `@prophetica/design-tokens` so the
 *   chart inherits the same source of truth as Tailwind / CSS — flipping a
 *   token there propagates here on next build. Alpha-derived colors are
 *   computed via `hexToRgba` rather than stored separately, so the line area
 *   tracks `colors.primary` without a parallel token to maintain.
 */

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function ProbabilityChart({ data, height = 280, ariaLabel }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Area"> | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const container = containerRef.current;

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: colors.canvas },
        textColor: colors["ink-muted-48"],
        fontFamily: fontFamilies.text,
      },
      grid: {
        vertLines: { visible: false },
        horzLines: { color: colors["divider-soft"], style: LineStyle.Solid },
      },
      rightPriceScale: {
        borderVisible: false,
        scaleMargins: { top: 0.2, bottom: 0.15 },
      },
      timeScale: {
        borderVisible: false,
        timeVisible: false,
      },
      crosshair: {
        vertLine: { color: colors["body-muted"], width: 1, style: LineStyle.Dashed, labelVisible: false },
        horzLine: { color: colors["body-muted"], width: 1, style: LineStyle.Dashed, labelVisible: true },
      },
      width: container.clientWidth,
      height,
      handleScale: { axisPressedMouseMove: false, mouseWheel: false, pinch: false },
      handleScroll: { mouseWheel: false, pressedMouseMove: false, vertTouchDrag: false, horzTouchDrag: true },
    });
    chartRef.current = chart;

    const series = chart.addSeries(AreaSeries, {
      lineColor: colors.primary,
      topColor: hexToRgba(colors.primary, 0.28),
      bottomColor: hexToRgba(colors.primary, 0.02),
      lineWidth: 2,
      priceFormat: { type: "custom", formatter: (p: number) => `${Math.round(p)}¢` },
    });
    seriesRef.current = series;
    series.setData(data);
    chart.timeScale().fitContent();

    const onResize = () => {
      if (containerRef.current) {
        chart.applyOptions({ width: containerRef.current.clientWidth });
      }
    };
    window.addEventListener("resize", onResize);

    return () => {
      window.removeEventListener("resize", onResize);
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, [data, height]);

  return <div ref={containerRef} className="w-full" aria-label={ariaLabel} />;
}
