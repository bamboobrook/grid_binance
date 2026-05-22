"use client";

import { useMemo, useState } from "react";
import type { MartingaleEquityPoint, MartingaleBacktestCandidateSummary, MartingaleTradeDetail } from "@/lib/api-types";

type RawChartPoint = MartingaleEquityPoint & { t?: number; timestamp_ms?: number; equity_quote?: number; drawdown_pct?: number };

interface EquityPoint {
  ts: number;
  equity: number;
}

interface DrawdownPoint {
  ts: number;
  drawdown: number;
}

/* ------------------------------------------------------------------ */
/*  Helpers                                                           */
/* ------------------------------------------------------------------ */

function fmtPct(v: number): string {
  return `${(v * 100).toFixed(1)}%`;
}

function fmtPctValue(v: number): string {
  return `${v.toFixed(2)}%`;
}

function fmtNum(v: number, decimals = 2): string {
  return v.toLocaleString(undefined, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

function readFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readPointTime(point: RawChartPoint): number | null {
  return readFiniteNumber(point.ts) ?? readFiniteNumber(point.t) ?? readFiniteNumber(point.timestamp_ms);
}

function normalizeEquityCurve(points: unknown): EquityPoint[] {
  if (!Array.isArray(points)) return [];
  return points
    .map((point) => {
      const rawPoint = point as RawChartPoint;
      const ts = readPointTime(rawPoint);
      const equity = readFiniteNumber(rawPoint.equity) ?? readFiniteNumber(rawPoint.equity_quote);
      return ts == null || equity == null ? null : { ts, equity };
    })
    .filter((point): point is EquityPoint => point != null)
    .sort((left, right) => left.ts - right.ts);
}

function normalizeDrawdownCurve(drawdownCurve: unknown, fallbackEquityCurve: unknown): DrawdownPoint[] {
  const rawPoints = Array.isArray(drawdownCurve) && drawdownCurve.length > 0 ? drawdownCurve : fallbackEquityCurve;
  if (!Array.isArray(rawPoints)) return [];
  return rawPoints
    .map((point) => {
      const rawPoint = point as RawChartPoint;
      const ts = readPointTime(rawPoint);
      const drawdown = readFiniteNumber(rawPoint.drawdown) ?? readFiniteNumber(rawPoint.drawdown_pct);
      return ts == null || drawdown == null ? null : { ts, drawdown };
    })
    .filter((point): point is DrawdownPoint => point != null)
    .sort((left, right) => left.ts - right.ts);
}

/* ------------------------------------------------------------------ */
/*  Wide interactive charts                                           */
/* ------------------------------------------------------------------ */

type InteractiveChartPoint = {
  ts: number;
  value: number;
  equity?: number;
};

function buildLinePath(points: { x: number; y: number }[]) {
  return points.map((point, index) => `${index === 0 ? "M" : "L"}${point.x.toFixed(1)},${point.y.toFixed(1)}`).join(" ");
}

function InteractiveLineChart({
  title,
  points,
  valueLabel,
  valueFormatter,
  stroke,
  fill,
  firstEquity,
}: {
  title: string;
  points: InteractiveChartPoint[];
  valueLabel: string;
  valueFormatter: (value: number) => string;
  stroke: string;
  fill: string;
  firstEquity?: number;
}) {
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  const width = 1000;
  const height = 260;
  const padX = 28;
  const padY = 22;
  const chartWidth = width - padX * 2;
  const chartHeight = height - padY * 2;
  const geometry = useMemo(() => {
    if (points.length < 2) return null;
    const xs = points.map((point) => point.ts);
    const ys = points.map((point) => point.value);
    const xMin = Math.min(...xs);
    const xMax = Math.max(...xs);
    const yMin = Math.min(...ys);
    const yMax = Math.max(...ys);
    const xRange = xMax - xMin || 1;
    const yRange = yMax - yMin || 1;
    const coords = points.map((point) => ({
      x: padX + ((point.ts - xMin) / xRange) * chartWidth,
      y: padY + chartHeight - ((point.value - yMin) / yRange) * chartHeight,
    }));
    const line = buildLinePath(coords);
    const area = `${line} L${coords[coords.length - 1].x.toFixed(1)},${padY + chartHeight} L${padX},${padY + chartHeight} Z`;
    return { coords, line, area };
  }, [points, chartWidth, chartHeight]);

  if (!geometry) {
    return (
      <div className="rounded-xl border border-border bg-background p-3 text-sm text-muted-foreground">
        {title}: No chart data
      </div>
    );
  }

  const activeIndex = hoverIndex ?? points.length - 1;
  const activePoint = points[activeIndex];
  const activeCoord = geometry.coords[activeIndex];
  const returnPct = firstEquity && activePoint.equity != null && firstEquity > 0
    ? ((activePoint.equity / firstEquity - 1) * 100).toFixed(2)
    : null;

  return (
    <div className="w-full rounded-xl border border-border bg-background p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <h4 className="text-sm font-medium">{title}</h4>
        <span className="text-xs text-muted-foreground">{points.length} points</span>
      </div>
      <div className="relative">
        <svg
          className="h-64 w-full touch-none select-none"
          viewBox={`0 0 ${width} ${height}`}
          onMouseLeave={() => setHoverIndex(null)}
          onMouseMove={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
            setHoverIndex(Math.round(ratio * (points.length - 1)));
          }}
        >
          <line x1={padX} y1={padY + chartHeight} x2={width - padX} y2={padY + chartHeight} stroke="currentColor" className="text-border" strokeWidth="1" />
          <path d={geometry.area} fill={fill} />
          <path d={geometry.line} fill="none" stroke={stroke} strokeWidth="2.5" />
          <line x1={activeCoord.x} x2={activeCoord.x} y1={padY} y2={padY + chartHeight} stroke="currentColor" className="text-muted-foreground/60" strokeDasharray="4 4" />
          <circle cx={activeCoord.x} cy={activeCoord.y} r="5" fill={stroke} stroke="white" strokeWidth="2" />
        </svg>
        <div
          className="pointer-events-none absolute top-4 z-10 min-w-48 rounded-lg border border-border bg-popover p-2 text-xs shadow-lg"
          style={{ left: `${Math.min(82, Math.max(2, (activeCoord.x / width) * 100))}%`, transform: activeCoord.x > width * 0.75 ? "translateX(-100%)" : undefined }}
        >
          <p className="font-medium">{new Date(activePoint.ts).toLocaleDateString()}</p>
          <p className="text-muted-foreground">{valueLabel}: <span className="text-foreground font-semibold">{valueFormatter(activePoint.value)}</span></p>
          {returnPct != null ? <p className="text-muted-foreground">收益: <span className="text-foreground font-semibold">{returnPct}%</span></p> : null}
        </div>
      </div>
    </div>
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
  const equityPoints = normalizeEquityCurve(equityCurve ?? summary?.equity_curve);
  const drawdownPoints = normalizeDrawdownCurve(summary?.drawdown_curve, equityCurve ?? summary?.equity_curve);
  const artifactPath = summary?.artifact_path;
  const maxDrawdownPct = summary?.max_drawdown_pct ?? (summary?.max_drawdown == null ? undefined : summary.max_drawdown * 100);
  const hasAnyChartData = equityPoints.length > 0 || drawdownPoints.length > 0;
  const hasSummaryMetrics =
    summary?.total_return_pct != null
    || maxDrawdownPct != null
    || summary?.score != null
    || Boolean(summary?.risk_summary_human);

  return (
    <div className="space-y-5 rounded-2xl border border-border bg-card p-4 lg:p-5">
      {!hasAnyChartData && artifactPath ? (
        <div className="rounded-lg border border-dashed border-border bg-secondary/20 p-3 text-sm text-muted-foreground">
          图表数据需要从 artifact 加载：{artifactPath}
        </div>
      ) : null}

      {!hasAnyChartData && !artifactPath ? (
        <div className="rounded-lg border border-dashed border-border bg-secondary/20 p-3 text-sm text-muted-foreground">
          图表数据缺失：该候选没有保存资金曲线或回撤曲线
        </div>
      ) : null}

      {equityPoints.length > 0 ? (
        <div className="space-y-2">
          <InteractiveLineChart
            title="资金曲线 / Equity curve"
            points={equityPoints.map((point) => ({ ts: point.ts, value: point.equity, equity: point.equity }))}
            valueLabel="资金"
            valueFormatter={(value) => fmtNum(value)}
            stroke="#2563eb"
            fill="rgba(37,99,235,0.10)"
            firstEquity={equityPoints[0]?.equity}
          />
          <div className="flex gap-4 text-xs text-muted-foreground">
            <span>Start: {fmtNum(equityPoints[0].equity)}</span>
            <span>Peak: {fmtNum(Math.max(...equityPoints.map((point) => point.equity)))}</span>
            <span>End: {fmtNum(equityPoints[equityPoints.length - 1].equity)}</span>
          </div>
        </div>
      ) : null}

      {drawdownPoints.length > 0 ? (
        <div className="space-y-2">
          <InteractiveLineChart
            title="回撤曲线 / Drawdown curve"
            points={drawdownPoints.map((point) => ({ ts: point.ts, value: point.drawdown }))}
            valueLabel="回撤"
            valueFormatter={(value) => fmtPctValue(value)}
            stroke="#ef4444"
            fill="rgba(239,68,68,0.10)"
          />
          {maxDrawdownPct != null ? (
            <div className="text-xs text-muted-foreground">
              Max Drawdown: <span className="text-red-600 font-semibold">{fmtPctValue(maxDrawdownPct)}</span>
            </div>
          ) : null}
        </div>
      ) : null}

      <div>
        <h4 className="text-sm font-medium mb-1">候选对比 / Candidate comparison</h4>
        {hasSummaryMetrics ? (
          <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
            <MetricCard label="Return" value={summary?.total_return_pct == null ? "—" : fmtPctValue(summary.total_return_pct)} />
            <MetricCard label="Max DD" value={maxDrawdownPct == null ? "—" : fmtPctValue(maxDrawdownPct)} />
            <MetricCard label="Score" value={summary?.score == null ? "—" : fmtNum(summary.score, 2)} />
            <MetricCard label="Risk" value={summary?.risk_summary_human || "—"} />
          </div>
        ) : (
          <span className="text-muted-foreground text-sm">暂无候选对比或风险摘要数据</span>
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

      {/* Trade details */}
      <div>
        <h4 className="text-sm font-medium mb-1">交易明细 / Trade details</h4>
        <TradeDetailsPreview trades={summary?.trades_preview ?? []} />
      </div>
    </div>
  );
}

function TradeDetailsPreview({ trades }: { trades: MartingaleTradeDetail[] }) {
  if (trades.length === 0) return <p className="text-xs text-muted-foreground">No trade details available.</p>;
  const preview = trades.slice(0, 20);
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="border-b border-border">
            <th className="px-1 py-0.5 text-left">Time</th>
            <th className="px-1 py-0.5 text-left">Symbol</th>
            <th className="px-1 py-0.5 text-left">Event</th>
            <th className="px-1 py-0.5 text-right">PnL</th>
          </tr>
        </thead>
        <tbody>
          {preview.map((trade, i) => (
            <tr key={i} className="border-b border-border/50">
              <td className="px-1 py-0.5">{new Date(trade.timestamp_ms).toLocaleDateString()}</td>
              <td className="px-1 py-0.5">{trade.symbol}</td>
              <td className="px-1 py-0.5">{trade.event_type}</td>
              <td className="px-1 py-0.5 text-right">{trade.realized_pnl_quote.toFixed(2)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {trades.length > 20 && <p className="text-xs text-muted-foreground mt-1">Showing 20 of {trades.length} trades.</p>}
    </div>
  );
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-secondary/20 p-2">
      <p className="text-muted-foreground">{label}</p>
      <p className="mt-1 truncate font-semibold" title={value}>{value}</p>
    </div>
  );
}
