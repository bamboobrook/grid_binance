"use client";

import { useMemo } from "react";

import { formatPnlWithCurrency } from "@/lib/ui/format";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type PnlDataPoint = {
  date: string;
  pnl: number;
};

type ChartPoint = PnlDataPoint & {
  hitWidth: number;
  x: number;
  y: number;
};

const CHART = {
  bottom: 166,
  left: 54,
  right: 18,
  top: 16,
  viewBoxHeight: 206,
  viewBoxWidth: 720,
};

export function PnlTrendChart({
  data,
  lang,
  height = 220,
}: {
  data: PnlDataPoint[];
  lang: UiLanguage;
  height?: number;
}) {
  const chart = useMemo(() => buildChartModel(data), [data]);

  if (data.length === 0) {
    return (
      <div
        className="flex items-center justify-center rounded-md border border-dashed border-border bg-background text-xs text-muted-foreground"
        style={{ height }}
      >
        {pickText(lang, "暂无收益数据", "No PnL data yet")}
      </div>
    );
  }

  return (
    <div className="rounded-md border border-border bg-background px-3 pb-3 pt-3">
      <div className="relative" style={{ height }}>
        <svg
          aria-label={pickText(lang, "收益趋势图", "PnL trend chart")}
          className="block h-full w-full text-primary"
          role="img"
          viewBox={`0 0 ${CHART.viewBoxWidth} ${CHART.viewBoxHeight}`}
        >
          <defs>
            <linearGradient id="dashboard-pnl-area" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor="currentColor" stopOpacity="0.26" />
              <stop offset="100%" stopColor="currentColor" stopOpacity="0.03" />
            </linearGradient>
          </defs>

          <text className="fill-muted-foreground text-[10px] font-bold" textAnchor="start" x={CHART.left} y="10">
            USDT
          </text>
          <line className="stroke-border" strokeWidth="1" x1={CHART.left} x2={CHART.left} y1={CHART.top} y2={CHART.bottom} />
          <line className="stroke-border" strokeWidth="1" x1={CHART.left} x2={CHART.viewBoxWidth - CHART.right} y1={CHART.bottom} y2={CHART.bottom} />

          {chart.yTicks.map((tick) => (
            <g key={tick.value}>
              <line className="stroke-border" strokeDasharray="4 4" strokeWidth="1" x1={CHART.left} x2={CHART.viewBoxWidth - CHART.right} y1={tick.y} y2={tick.y} />
              <text className="fill-muted-foreground text-[11px] font-semibold" dominantBaseline="middle" textAnchor="end" x={CHART.left - 10} y={tick.y}>
                {formatAxisValue(tick.value)}
              </text>
            </g>
          ))}

          <path d={chart.areaPath} fill="url(#dashboard-pnl-area)" />
          <path className="fill-none stroke-primary" d={chart.linePath} strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.5" />

          {chart.xLabels.map((label) => (
            <text className="fill-muted-foreground text-[11px] font-semibold" key={`${label.date}-${label.index}`} textAnchor={label.anchor} x={label.x} y={CHART.viewBoxHeight - 8}>
              {label.date}
            </text>
          ))}

          {chart.points.map((point) => (
            <PointHoverLayer key={point.date} lang={lang} point={point} />
          ))}
        </svg>
      </div>
    </div>
  );
}

function PointHoverLayer({
  lang,
  point,
}: {
  lang: UiLanguage;
  point: ChartPoint;
}) {
  const tooltipLabel = `${point.date} ${pickText(lang, "当日收益", "Daily PnL")} ${formatPnlWithCurrency(point.pnl)}`;

  return (
    <g className="group">
      <rect
        aria-label={tooltipLabel}
        className="cursor-crosshair fill-transparent"
        height={CHART.bottom - CHART.top}
        pointerEvents="all"
        role="presentation"
        width={point.hitWidth}
        x={point.x - point.hitWidth / 2}
        y={CHART.top}
      >
        <title>{tooltipLabel}</title>
      </rect>
      <g className="pointer-events-none opacity-0 transition-opacity duration-150 group-hover:opacity-100">
        <line className="stroke-primary" opacity="0.8" strokeDasharray="5 5" strokeWidth="1.2" x1={point.x} x2={point.x} y1={CHART.top} y2={CHART.bottom} />
        <line className="stroke-primary" opacity="0.45" strokeDasharray="5 5" strokeWidth="1" x1={CHART.left} x2={point.x} y1={point.y} y2={point.y} />
        <rect className="fill-primary" height="22" rx="5" width="50" x="0" y={getYAxisMarkerY(point)} />
        <text className="fill-primary-foreground text-[10px] font-bold" dominantBaseline="middle" textAnchor="end" x={CHART.left - 10} y={getYAxisMarkerY(point) + 11}>
          {formatAxisValue(point.pnl)}
        </text>
        <circle className="fill-background stroke-primary" cx={point.x} cy={point.y} r="4.5" strokeWidth="2" />
        <TooltipBox lang={lang} point={point} />
      </g>
    </g>
  );
}

function TooltipBox({
  lang,
  point,
}: {
  lang: UiLanguage;
  point: ChartPoint;
}) {
  const box = getTooltipBox(point);

  return (
    <g data-pnl-tooltip>
      <rect className="fill-popover stroke-border" height={box.height} rx="6" width={box.width} x={box.x} y={box.y} />
      <text className="fill-foreground text-[12px] font-bold" x={box.x + 10} y={box.y + 18}>
        {point.date}
      </text>
      <text className="fill-muted-foreground text-[11px]" x={box.x + 10} y={box.y + 38}>
        {pickText(lang, "当日收益", "Daily PnL")}
      </text>
      <text className={point.pnl >= 0 ? "fill-emerald-500 text-[12px] font-bold" : "fill-red-500 text-[12px] font-bold"} textAnchor="end" x={box.x + box.width - 10} y={box.y + 38}>
        {formatPnlWithCurrency(point.pnl)}
      </text>
    </g>
  );
}

function buildChartModel(data: PnlDataPoint[]) {
  const values = data.map((item) => item.pnl);
  const minValue = Math.min(...values, 0);
  const maxValue = Math.max(...values, 0);
  const padded = padRange(minValue, maxValue);
  const plotWidth = CHART.viewBoxWidth - CHART.left - CHART.right;
  const plotHeight = CHART.bottom - CHART.top;
  const span = padded.max - padded.min || 1;
  const hitWidth = data.length === 1 ? plotWidth : plotWidth / (data.length - 1);
  const points = data.map((item, index) => {
    const x = data.length === 1 ? CHART.left + plotWidth / 2 : CHART.left + (index / (data.length - 1)) * plotWidth;
    const y = CHART.bottom - ((item.pnl - padded.min) / span) * plotHeight;
    return { ...item, hitWidth, x, y };
  });

  return {
    areaPath: `${buildSmoothPath(points)} L ${CHART.viewBoxWidth - CHART.right} ${CHART.bottom} L ${CHART.left} ${CHART.bottom} Z`,
    linePath: buildSmoothPath(points),
    points,
    xLabels: selectDateLabels(points, 6),
    yTicks: buildYAxisTicks(padded.min, padded.max, 4),
  };
}

function getYAxisMarkerY(point: ChartPoint) {
  return clamp(point.y - 11, CHART.top, CHART.bottom - 22);
}

function getTooltipBox(point: ChartPoint) {
  const width = 154;
  const height = 52;
  const x = clamp(point.x - width / 2, CHART.left + 4, CHART.viewBoxWidth - CHART.right - width - 4);
  const y = point.y < CHART.top + height + 16 ? point.y + 14 : point.y - height - 14;

  return {
    height,
    width,
    x,
    y: clamp(y, 4, CHART.bottom - height - 4),
  };
}

function padRange(min: number, max: number) {
  const span = max - min || Math.max(Math.abs(max), 1);
  return {
    max: max + span * 0.12,
    min: min - span * 0.08,
  };
}

function buildYAxisTicks(min: number, max: number, count: number) {
  return Array.from({ length: count }, (_, index) => {
    const value = max - ((max - min) / (count - 1)) * index;
    const y = CHART.top + ((max - value) / (max - min || 1)) * (CHART.bottom - CHART.top);
    return { value, y };
  });
}

function selectDateLabels(points: ChartPoint[], maxLabels: number) {
  if (points.length <= maxLabels) {
    return points.map((point, index) => ({
      anchor: anchorForIndex(index, points.length),
      date: point.date,
      index,
      x: point.x,
    }));
  }
  const step = (points.length - 1) / (maxLabels - 1);
  return Array.from({ length: maxLabels }, (_, itemIndex) => {
    const index = Math.min(points.length - 1, Math.round(itemIndex * step));
    const point = points[index];
    return {
      anchor: anchorForIndex(itemIndex, maxLabels),
      date: point.date,
      index,
      x: point.x,
    };
  });
}

function anchorForIndex(index: number, count: number): "start" | "middle" | "end" {
  if (index === 0) return "start";
  if (index === count - 1) return "end";
  return "middle";
}

function buildSmoothPath(points: ChartPoint[]) {
  if (points.length === 0) {
    return "";
  }
  if (points.length === 1) {
    return `M ${points[0].x.toFixed(2)} ${points[0].y.toFixed(2)}`;
  }
  const commands = [`M ${points[0].x.toFixed(2)} ${points[0].y.toFixed(2)}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const current = points[index];
    const next = points[index + 1];
    const previous = points[index - 1] ?? current;
    const afterNext = points[index + 2] ?? next;
    const cp1x = current.x + (next.x - previous.x) / 6;
    const cp1y = current.y + (next.y - previous.y) / 6;
    const cp2x = next.x - (afterNext.x - current.x) / 6;
    const cp2y = next.y - (afterNext.y - current.y) / 6;
    commands.push(
      `C ${cp1x.toFixed(2)} ${cp1y.toFixed(2)}, ${cp2x.toFixed(2)} ${cp2y.toFixed(2)}, ${next.x.toFixed(2)} ${next.y.toFixed(2)}`,
    );
  }
  return commands.join(" ");
}

function formatAxisValue(value: number) {
  const sign = value > 0 ? "+" : "";
  const abs = Math.abs(value);
  if (abs >= 1000) {
    return `${sign}${(value / 1000).toFixed(1)}k`;
  }
  if (abs >= 100) {
    return `${sign}${value.toFixed(0)}`;
  }
  if (abs >= 10) {
    return `${sign}${value.toFixed(1)}`;
  }
  return `${sign}${value.toFixed(2)}`;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}
