"use client";

import { useMemo } from "react";
import type { MartingaleEquityPoint, MartingaleBacktestCandidateSummary } from "@/lib/api-types";

/* ------------------------------------------------------------------ */
/*  Helpers                                                           */
/* ------------------------------------------------------------------ */

function fmtPct(v: number): string {
  return `${(v * 100).toFixed(1)}%`;
}

function fmtNum(v: number, decimals = 2): string {
  return v.toLocaleString(undefined, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

/* ------------------------------------------------------------------ */
/*  Mini sparkline from equity curve data                             */
/* ------------------------------------------------------------------ */

interface EquitySparklineProps {
  points: MartingaleEquityPoint[];
  width?: number;
  height?: number;
  stroke?: string;
  fill?: string;
}

function EquitySparkline({
  points,
  width = 280,
  height = 80,
  stroke = "var(--chart-1, #3b82f6)",
  fill = "var(--chart-1-alpha, rgba(59,130,246,0.10))",
}: EquitySparklineProps) {
  const path = useMemo(() => {
    if (!points || points.length < 2) return null;
    const xs = points.map((p) => p.ts);
    const ys = points.map((p) => p.equity);
    const xMin = Math.min(...xs);
    const xMax = Math.max(...xs);
    const yMin = Math.min(...ys);
    const yMax = Math.max(...ys);
    const xRange = xMax - xMin || 1;
    const yRange = yMax - yMin || 1;
    const pad = 4;
    const w = width - pad * 2;
    const h = height - pad * 2;

    const coords = points.map((p) => ({
      x: pad + ((p.ts - xMin) / xRange) * w,
      y: pad + h - ((p.equity - yMin) / yRange) * h,
    }));

    const line = coords.map((c, i) => `${i === 0 ? "M" : "L"}${c.x.toFixed(1)},${c.y.toFixed(1)}`).join(" ");
    const area = `${line} L${coords[coords.length - 1].x.toFixed(1)},${pad + h} L${pad},${pad + h} Z`;
    return { line, area };
  }, [points, width, height]);

  if (!path) {
    return (
      <svg width={width} height={height} className="opacity-40">
        <text x={width / 2} y={height / 2} textAnchor="middle" className="fill-muted-foreground text-xs">
          No equity data
        </text>
      </svg>
    );
  }

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
      <path d={path.area} fill={fill} />
      <path d={path.line} fill="none" stroke={stroke} strokeWidth={1.5} />
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/*  Drawdown sparkline                                                */
/* ------------------------------------------------------------------ */

interface DrawdownSparklineProps {
  points: MartingaleEquityPoint[];
  width?: number;
  height?: number;
}

function DrawdownSparkline({ points, width = 280, height = 60 }: DrawdownSparklineProps) {
  const path = useMemo(() => {
    if (!points || points.length < 2) return null;
    const xs = points.map((p) => p.ts);
    const ys = points.map((p) => p.drawdown ?? 0);
    const xMin = Math.min(...xs);
    const xMax = Math.max(...xs);
    const yMin = Math.min(...ys);
    const yMax = 0;
    const xRange = xMax - xMin || 1;
    const yRange = yMax - yMin || 1;
    const pad = 4;
    const w = width - pad * 2;
    const h = height - pad * 2;

    const coords = points.map((p) => ({
      x: pad + ((p.ts - xMin) / xRange) * w,
      y: pad + h - (((p.drawdown ?? 0) - yMin) / yRange) * h,
    }));

    const line = coords.map((c, i) => `${i === 0 ? "M" : "L"}${c.x.toFixed(1)},${c.y.toFixed(1)}`).join(" ");
    const area = `${line} L${coords[coords.length - 1].x.toFixed(1)},${pad + h} L${pad},${pad + h} Z`;
    return { line, area };
  }, [points, width, height]);

  if (!path) {
    return (
      <svg width={width} height={height} className="opacity-40">
        <text x={width / 2} y={height / 2} textAnchor="middle" className="fill-muted-foreground text-xs">
          No drawdown data
        </text>
      </svg>
    );
  }

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
      <path d={path.area} fill="rgba(239,68,68,0.10)" />
      <path d={path.line} fill="none" stroke="#ef4444" strokeWidth={1.5} />
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/*  Stress window badges from real data                              */
/* ------------------------------------------------------------------ */

interface StressWindowsProps {
  summary: MartingaleBacktestCandidateSummary;
}

function StressWindows({ summary }: StressWindowsProps) {
  const windows = summary?.stress_window_scores;
  if (!windows || Object.keys(windows).length === 0) {
    return <span className="text-muted-foreground text-sm">No stress window data</span>;
  }

  const labels: Record<string, string> = {
    flash_crash: "Flash Crash",
    prolonged_bear: "Prolonged Bear",
    high_volatility: "High Volatility",
    range_bound: "Range Bound",
    sudden_spike: "Sudden Spike",
    liquidity_crisis: "Liquidity Crisis",
  };

  return (
    <div className="flex flex-wrap gap-2">
      {Object.entries(windows).map(([key, score]) => (
        <div
          key={key}
          className="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs"
        >
          <span className="text-muted-foreground">{labels[key] ?? key}</span>
          <span className={`font-semibold ${score >= 0.7 ? "text-green-600" : score >= 0.4 ? "text-yellow-600" : "text-red-600"}`}>
            {fmtPct(score)}
          </span>
        </div>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Stop-loss event timeline                                          */
/* ------------------------------------------------------------------ */

interface StopLossEventsProps {
  events: { ts: number; symbol: string; reason: string; loss_pct: number }[];
}

function StopLossEvents({ events }: StopLossEventsProps) {
  if (!events || events.length === 0) {
    return <span className="text-muted-foreground text-sm">No stop-loss events</span>;
  }

  return (
    <div className="space-y-1 max-h-40 overflow-y-auto">
      {events.map((ev, i) => (
        <div key={i} className="flex items-center gap-2 text-xs">
          <span className="text-muted-foreground w-28 shrink-0">
            {new Date(ev.ts).toLocaleDateString()}
          </span>
          <span className="font-medium">{ev.symbol}</span>
          <span className="text-muted-foreground">{ev.reason}</span>
          <span className="text-red-600 font-semibold ml-auto">{fmtPct(ev.loss_pct)}</span>
        </div>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Main exported component                                           */
/* ------------------------------------------------------------------ */

export interface BacktestChartsProps {
  summary: MartingaleBacktestCandidateSummary;
  equityCurve?: MartingaleEquityPoint[];
  stopLossEvents?: { ts: number; symbol: string; reason: string; loss_pct: number }[];
}

export function BacktestCharts({ summary, equityCurve, stopLossEvents }: BacktestChartsProps) {
  const points = equityCurve ?? summary?.equity_curve ?? [];

  return (
    <div className="space-y-4">
      {/* Equity curve */}
      <div>
        <h4 className="text-sm font-medium mb-1">Equity Curve</h4>
        <EquitySparkline points={points} />
        {points.length > 0 && (
          <div className="flex gap-4 text-xs text-muted-foreground mt-1">
            <span>Start: {fmtNum(points[0].equity)}</span>
            <span>Peak: {fmtNum(Math.max(...points.map((p) => p.equity)))}</span>
            <span>End: {fmtNum(points[points.length - 1].equity)}</span>
          </div>
        )}
      </div>

      {/* Drawdown curve */}
      <div>
        <h4 className="text-sm font-medium mb-1">Drawdown</h4>
        <DrawdownSparkline points={points} />
        {summary?.max_drawdown != null && (
          <div className="text-xs text-muted-foreground mt-1">
            Max Drawdown: <span className="text-red-600 font-semibold">{fmtPct(summary.max_drawdown)}</span>
          </div>
        )}
      </div>

      {/* Stress windows */}
      <div>
        <h4 className="text-sm font-medium mb-1">Stress Windows</h4>
        <StressWindows summary={summary} />
      </div>

      {/* Stop-loss events */}
      <div>
        <h4 className="text-sm font-medium mb-1">Stop-Loss Events</h4>
        <StopLossEvents events={stopLossEvents ?? summary?.stop_loss_events ?? []} />
      </div>
    </div>
  );
}
