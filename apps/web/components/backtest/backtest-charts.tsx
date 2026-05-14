"use client";

import { useMemo } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { MartingaleEquityPoint, MartingaleBacktestCandidateSummary } from "@/lib/api-types";

type RawChartPoint = MartingaleEquityPoint & { t?: number };

type AllocationPoint = {
  timestamp_ms: number;
  symbol: string;
  long_weight_pct: number;
  short_weight_pct: number;
  action?: string;
  reason?: string;
  in_cooldown?: boolean;
};

type RegimePoint = {
  timestamp_ms?: number;
  btc_regime?: string;
  symbol_regime?: string;
  symbol?: string;
};

type CostSummary = {
  fee_quote?: number;
  slippage_quote?: number;
  stop_loss_quote?: number;
  forced_exit_quote?: number;
  rebalance_count?: number;
  forced_exit_count?: number;
  average_allocation_hold_hours?: number;
};

type DynamicAllocationSummary = MartingaleBacktestCandidateSummary & {
  allocation_curve?: unknown;
  artifact?: { allocation_curve?: unknown };
  candidate_artifact?: { allocation_curve?: unknown };
  regime_timeline?: unknown;
  cost_summary?: CostSummary;
};

interface EquityPoint {
  ts: number;
  equity: number;
}

interface DrawdownPoint {
  ts: number;
  drawdown: number;
}

interface EquityChartPoint extends EquityPoint {
  date: string;
  returnPct: number;
}

interface DrawdownChartPoint extends DrawdownPoint {
  date: string;
  drawdownPct: number;
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

function fmtDateTime(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function fmtShortDate(ts: number): string {
  return new Date(ts).toLocaleDateString(undefined, { month: "2-digit", day: "2-digit" });
}

function readFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readObject(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function readString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function readBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function readPointTime(point: RawChartPoint): number | null {
  return readFiniteNumber(point.ts) ?? readFiniteNumber(point.t);
}

function normalizeEquityCurve(points: unknown): EquityPoint[] {
  if (!Array.isArray(points)) return [];
  return points
    .map((point) => {
      const rawPoint = point as RawChartPoint;
      const ts = readPointTime(rawPoint);
      const equity = readFiniteNumber(rawPoint.equity);
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
      const drawdown = readFiniteNumber(rawPoint.drawdown);
      return ts == null || drawdown == null ? null : { ts, drawdown };
    })
    .filter((point): point is DrawdownPoint => point != null)
    .sort((left, right) => left.ts - right.ts);
}

function normalizeAllocationCurve(summary: DynamicAllocationSummary): AllocationPoint[] {
  const rawCurve = Array.isArray(summary.allocation_curve) && summary.allocation_curve.length > 0
    ? summary.allocation_curve
    : Array.isArray(summary.artifact?.allocation_curve) && summary.artifact.allocation_curve.length > 0
      ? summary.artifact.allocation_curve
      : summary.candidate_artifact?.allocation_curve;

  if (!Array.isArray(rawCurve)) return [];
  return rawCurve
    .map((point): AllocationPoint | null => {
      const object = readObject(point);
      if (!object) return null;
      const timestamp = readFiniteNumber(object.timestamp_ms) ?? readFiniteNumber(object.ts) ?? readFiniteNumber(object.t);
      const longWeight = readFiniteNumber(object.long_weight_pct);
      const shortWeight = readFiniteNumber(object.short_weight_pct);
      if (timestamp == null || longWeight == null || shortWeight == null) return null;
      return {
        timestamp_ms: timestamp,
        symbol: readString(object.symbol) || summary.symbol || "—",
        long_weight_pct: longWeight,
        short_weight_pct: shortWeight,
        action: readString(object.action),
        reason: readString(object.reason),
        in_cooldown: readBoolean(object.in_cooldown),
      } satisfies AllocationPoint;
    })
    .filter((point): point is AllocationPoint => point != null)
    .sort((left, right) => left.timestamp_ms - right.timestamp_ms);
}

function normalizeRegimeTimeline(summary: DynamicAllocationSummary): RegimePoint[] {
  if (!Array.isArray(summary.regime_timeline)) return [];
  return summary.regime_timeline
    .map((point): RegimePoint | null => {
      const object = readObject(point);
      if (!object) return null;
      return {
        timestamp_ms: readFiniteNumber(object.timestamp_ms) ?? readFiniteNumber(object.ts) ?? undefined,
        btc_regime: readString(object.btc_regime),
        symbol_regime: readString(object.symbol_regime),
        symbol: readString(object.symbol),
      } satisfies RegimePoint;
    })
    .filter((point): point is RegimePoint => point != null);
}

interface EquitySparklineProps {
  points: EquityPoint[];
  height?: number;
}

function EquitySparkline({
  points,
  height = 220,
}: EquitySparklineProps) {
  const data = useMemo<EquityChartPoint[]>(() => {
    const startEquity = points[0]?.equity ?? 0;
    return points.map((point) => ({
      ...point,
      date: fmtShortDate(point.ts),
      returnPct: startEquity > 0 ? ((point.equity - startEquity) / startEquity) * 100 : 0,
    }));
  }, [points]);

  if (data.length < 2) {
    return (
      <div className="flex items-center justify-center text-xs text-muted-foreground opacity-60" style={{ height }}>
          No equity data
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data} margin={{ top: 8, right: 12, left: 4, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" opacity={0.55} />
        <XAxis dataKey="date" minTickGap={28} tick={{ fontSize: 10, fill: "var(--muted-foreground)" }} tickLine={false} />
        <YAxis
          domain={["dataMin", "dataMax"]}
          tick={{ fontSize: 10, fill: "var(--muted-foreground)" }}
          tickFormatter={(value: number) => fmtNum(value, 0)}
          tickLine={false}
          width={52}
        />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--card)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 }}
          labelFormatter={(_, payload) => {
            const point = payload?.[0]?.payload as EquityChartPoint | undefined;
            return point ? fmtDateTime(point.ts) : "—";
          }}
          formatter={(value, name, item) => {
            const point = item.payload as EquityChartPoint;
            if (name === "equity") return [fmtNum(Number(value)), "资金"];
            return [`${fmtNum(point.returnPct, 2)}%`, "相对起点"];
          }}
        />
        <Area type="monotone" dataKey="equity" stroke="#2563eb" fill="#2563eb" fillOpacity={0.14} strokeWidth={2} name="equity" />
        <Line type="monotone" dataKey="returnPct" stroke="transparent" dot={false} activeDot={false} name="returnPct" />
      </AreaChart>
    </ResponsiveContainer>
  );
}

/* ------------------------------------------------------------------ */
/*  Drawdown sparkline                                                */
/* ------------------------------------------------------------------ */

interface DrawdownSparklineProps {
  points: DrawdownPoint[];
  height?: number;
}

function DrawdownSparkline({ points, height = 180 }: DrawdownSparklineProps) {
  const data = useMemo<DrawdownChartPoint[]>(() => {
    return points.map((point) => ({
      ...point,
      date: fmtShortDate(point.ts),
      drawdownPct: point.drawdown * 100,
    }));
  }, [points]);

  if (data.length < 2) {
    return (
      <div className="flex items-center justify-center text-xs text-muted-foreground opacity-60" style={{ height }}>
          No drawdown data
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data} margin={{ top: 8, right: 12, left: 4, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" opacity={0.55} />
        <XAxis dataKey="date" minTickGap={28} tick={{ fontSize: 10, fill: "var(--muted-foreground)" }} tickLine={false} />
        <YAxis
          domain={["dataMin", 0]}
          tick={{ fontSize: 10, fill: "var(--muted-foreground)" }}
          tickFormatter={(value: number) => `${fmtNum(value, 1)}%`}
          tickLine={false}
          width={52}
        />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--card)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 }}
          labelFormatter={(_, payload) => {
            const point = payload?.[0]?.payload as DrawdownChartPoint | undefined;
            return point ? fmtDateTime(point.ts) : "—";
          }}
          formatter={(value) => [`${fmtNum(Number(value), 2)}%`, "回撤"]}
        />
        <Area type="monotone" dataKey="drawdownPct" stroke="#ef4444" fill="#ef4444" fillOpacity={0.12} strokeWidth={2} />
      </AreaChart>
    </ResponsiveContainer>
  );
}

function AllocationChart({ points }: { points: AllocationPoint[] }) {
  const data = useMemo(() => points.map((point) => ({
    ...point,
    date: fmtShortDate(point.timestamp_ms),
  })), [points]);

  if (data.length < 2) {
    return <span className="text-muted-foreground text-sm">No allocation data</span>;
  }

  return (
    <ResponsiveContainer width="100%" height={190}>
      <AreaChart data={data} margin={{ top: 8, right: 12, left: 4, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" opacity={0.55} />
        <XAxis dataKey="date" minTickGap={28} tick={{ fontSize: 10, fill: "var(--muted-foreground)" }} tickLine={false} />
        <YAxis
          domain={[0, 100]}
          tick={{ fontSize: 10, fill: "var(--muted-foreground)" }}
          tickFormatter={(value: number) => `${fmtNum(value, 0)}%`}
          tickLine={false}
          width={48}
        />
        <Tooltip
          contentStyle={{ backgroundColor: "var(--card)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 }}
          labelFormatter={(_, payload) => {
            const point = payload?.[0]?.payload as AllocationPoint | undefined;
            return point ? fmtDateTime(point.timestamp_ms) : "—";
          }}
          formatter={(value, name, item) => {
            const point = item.payload as AllocationPoint;
            const label = name === "long_weight_pct" ? "Long %" : "Short %";
            return [
              `${fmtNum(Number(value), 2)}% · ${point.action || "—"} · ${point.reason || "—"} · cooldown=${point.in_cooldown ? "yes" : "no"}`,
              label,
            ];
          }}
        />
        <Area type="monotone" dataKey="long_weight_pct" name="long_weight_pct" stroke="#16a34a" fill="#16a34a" fillOpacity={0.12} strokeWidth={2} />
        <Area type="monotone" dataKey="short_weight_pct" name="short_weight_pct" stroke="#dc2626" fill="#dc2626" fillOpacity={0.10} strokeWidth={2} />
      </AreaChart>
    </ResponsiveContainer>
  );
}

function RegimeSummaryCards({ regimes }: { regimes: RegimePoint[] }) {
  if (regimes.length === 0) {
    return <span className="text-muted-foreground text-sm">No regime timeline data</span>;
  }
  const latest = regimes[regimes.length - 1];
  const btcRegimes = new Set(regimes.map((point) => point.btc_regime).filter(Boolean)).size;
  const symbolRegimes = new Set(regimes.map((point) => point.symbol_regime).filter(Boolean)).size;
  return (
    <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
      <MetricCard label="btc_regime" value={latest.btc_regime || "—"} />
      <MetricCard label="symbol_regime" value={latest.symbol_regime || "—"} />
      <MetricCard label="BTC Regime Types" value={String(btcRegimes)} />
      <MetricCard label="Symbol Regime Types" value={String(symbolRegimes)} />
    </div>
  );
}

function CostSummaryCards({ costSummary }: { costSummary?: CostSummary }) {
  if (!costSummary) {
    return <span className="text-muted-foreground text-sm">No cost_summary data</span>;
  }
  return (
    <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
      <MetricCard label="fee_quote" value={formatCost(costSummary.fee_quote)} />
      <MetricCard label="slippage_quote" value={formatCost(costSummary.slippage_quote)} />
      <MetricCard label="stop_loss_quote" value={formatCost(costSummary.stop_loss_quote)} />
      <MetricCard label="forced_exit_quote" value={formatCost(costSummary.forced_exit_quote)} />
      <MetricCard label="rebalance_count" value={formatCount(costSummary.rebalance_count)} />
      <MetricCard label="forced_exit_count" value={formatCount(costSummary.forced_exit_count)} />
      <MetricCard label="average_allocation_hold_hours" value={formatHours(costSummary.average_allocation_hold_hours)} />
    </div>
  );
}

function formatCost(value: number | undefined) {
  return value == null ? "—" : `${fmtNum(value, 2)} USDT`;
}

function formatCount(value: number | undefined) {
  return value == null ? "—" : fmtNum(value, 0);
}

function formatHours(value: number | undefined) {
  return value == null ? "—" : `${fmtNum(value, 2)}h`;
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
  const dynamicSummary = summary as DynamicAllocationSummary;
  const equityPoints = normalizeEquityCurve(equityCurve ?? summary?.equity_curve);
  const drawdownPoints = normalizeDrawdownCurve(summary?.drawdown_curve, equityCurve ?? summary?.equity_curve);
  const allocationPoints = normalizeAllocationCurve(dynamicSummary);
  const regimeTimeline = normalizeRegimeTimeline(dynamicSummary);
  const artifactPath = summary?.artifact_path;
  const maxDrawdownPct = summary?.max_drawdown_pct ?? (summary?.max_drawdown == null ? undefined : summary.max_drawdown * 100);
  const hasAnyChartData = equityPoints.length > 0 || drawdownPoints.length > 0;
  const hasSummaryMetrics =
    summary?.total_return_pct != null
    || summary?.annualized_return_pct != null
    || maxDrawdownPct != null
    || summary?.score != null
    || Boolean(summary?.risk_summary_human);
  const tradeEvents = summary?.sampled_trade_events ?? summary?.trade_events ?? [];
  const coverage = summary?.data_coverage;

  return (
    <div className="space-y-4">
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
        <div>
          <h4 className="text-sm font-medium mb-1">资金曲线 / Equity curve</h4>
          <EquitySparkline points={equityPoints} />
          <div className="flex gap-4 text-xs text-muted-foreground mt-1">
            <span>Start: {fmtNum(equityPoints[0].equity)}</span>
            <span>Peak: {fmtNum(Math.max(...equityPoints.map((p) => p.equity)))}</span>
            <span>End: {fmtNum(equityPoints[equityPoints.length - 1].equity)}</span>
          </div>
        </div>
      ) : null}

      {coverage ? (
        <div className="rounded-lg border border-border bg-secondary/20 p-3 text-xs text-muted-foreground">
          <span className="font-medium text-foreground">数据覆盖：</span>
          interval={coverage.interval ?? "—"} · bars={coverage.bar_count ?? "—"} · aggTrades={coverage.agg_trade_count ?? "—"}
          {coverage.first_bar_ms && coverage.last_bar_ms ? ` · ${new Date(coverage.first_bar_ms).toLocaleDateString()} → ${new Date(coverage.last_bar_ms).toLocaleDateString()}` : ""}
          {coverage.used_full_minute_coverage ? " · 已使用 1m 全量覆盖" : " · 请检查是否为全量 1m 数据"}
          {summary?.backtest_years != null ? ` · ${summary.backtest_years.toFixed(2)} 年` : ""}
        </div>
      ) : null}

      {drawdownPoints.length > 0 ? (
        <div>
          <h4 className="text-sm font-medium mb-1">回撤曲线 / Drawdown curve</h4>
          <DrawdownSparkline points={drawdownPoints} />
          {maxDrawdownPct != null && (
          <div className="text-xs text-muted-foreground mt-1">
            Max Drawdown: <span className="text-red-600 font-semibold">{fmtPctValue(maxDrawdownPct)}</span>
          </div>
          )}
        </div>
      ) : null}

      <div>
        <h4 className="text-sm font-medium mb-1">Long/Short Allocation</h4>
        <AllocationChart points={allocationPoints} />
      </div>

      <div>
        <h4 className="text-sm font-medium mb-1">Regime Summary</h4>
        <RegimeSummaryCards regimes={regimeTimeline} />
      </div>

      <div>
        <h4 className="text-sm font-medium mb-1">Cost Summary</h4>
        <CostSummaryCards costSummary={dynamicSummary.cost_summary} />
      </div>

      <div>
        <h4 className="text-sm font-medium mb-1">候选对比 / Candidate comparison</h4>
        {hasSummaryMetrics ? (
          <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
            <MetricCard label="Total Return" value={summary?.total_return_pct == null ? "—" : fmtPctValue(summary.total_return_pct)} />
            <MetricCard label="Annualized" value={summary?.annualized_return_pct == null ? "—" : fmtPctValue(summary.annualized_return_pct)} />
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

      <div>
        <h4 className="text-sm font-medium mb-1">交易明细 / Trade events</h4>
        {tradeEvents.length > 0 ? (
          <>
            <p className="mb-2 text-xs text-muted-foreground">按全周期均匀抽样展示，包含首尾交易事件；完整统计见交易数与资金曲线。</p>
          <div className="max-h-56 overflow-y-auto rounded-lg border border-border">
            {tradeEvents.slice(0, 80).map((event, index) => (
              <div className="grid grid-cols-[110px_90px_90px_minmax(0,1fr)] gap-2 border-b border-border px-3 py-2 text-xs last:border-b-0" key={`${event.ts}-${event.type}-${index}`}>
                <span className="text-muted-foreground">{new Date(event.ts).toLocaleDateString()}</span>
                <span className="font-medium">{event.type}</span>
                <span>{event.symbol}</span>
                <span className="truncate text-muted-foreground" title={event.detail ?? ""}>{event.detail ?? "—"}</span>
              </div>
            ))}
          </div>
          </>
        ) : (
          <span className="text-muted-foreground text-sm">暂无交易明细</span>
        )}
      </div>
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
