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
import type { ProbabilityPoint } from "@/lib/mock-markets";

interface Props {
  data: ProbabilityPoint[];
  /** Height in px; the chart fills container width responsively. */
  height?: number;
  ariaLabel?: string;
}

/*
 * YES-price-over-time chart powered by Lightweight Charts (TradingView, MIT).
 *
 * Why a client component: createChart binds to a DOM ref and only runs in the
 * browser. Lightweight Charts has no SSR support (per SPEC-014 §3.2 we accept
 * this — chart is supplementary, not blocking).
 *
 * Theme tokens come from design/DESIGN.md via CSS custom properties:
 *   - line / area:   --proph-color-primary (#0066cc)
 *   - background:    --proph-color-canvas (#ffffff)
 *   - text:          --proph-color-ink (#1d1d1f)
 *   - grid horiz:    --proph-color-divider-soft (#f0f0f0)
 *
 * We hardcode the resolved hex values here because Lightweight Charts options
 * are JS literals, not CSS strings — they don't follow `var(--…)` updates.
 * If DESIGN.md tokens change, update here too.
 */
export function ProbabilityChart({ data, height = 280, ariaLabel = "YES 價格走勢" }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Area"> | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const container = containerRef.current;

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: "#ffffff" },
        textColor: "#7a7a7a",
        fontFamily: "SF Pro Text, system-ui, -apple-system, sans-serif",
      },
      grid: {
        vertLines: { visible: false },
        horzLines: { color: "#f0f0f0", style: LineStyle.Solid },
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
        vertLine: { color: "#cccccc", width: 1, style: LineStyle.Dashed, labelVisible: false },
        horzLine: { color: "#cccccc", width: 1, style: LineStyle.Dashed, labelVisible: true },
      },
      width: container.clientWidth,
      height,
      handleScale: { axisPressedMouseMove: false, mouseWheel: false, pinch: false },
      handleScroll: { mouseWheel: false, pressedMouseMove: false, vertTouchDrag: false, horzTouchDrag: true },
    });
    chartRef.current = chart;

    const series = chart.addSeries(AreaSeries, {
      lineColor: "#0066cc",
      topColor: "rgba(0, 102, 204, 0.28)",
      bottomColor: "rgba(0, 102, 204, 0.02)",
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
