"use client";

import { useEffect, useState, type ReactNode } from "react";
import { Activity, BarChart3, Layers3, TrendingUp } from "lucide-react";

import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export type StrategyPreviewLevel = {
  entryPrice: string;
  quantity: string;
  spacingPercent: string | null;
  takeProfitPercent: string;
  trailingPercent: string | null;
};

type Props = {
  amountMode: "quote" | "base";
  coveredRangePercent: string;
  generation: "arithmetic" | "geometric" | "custom";
  gridCount: string;
  lang: UiLanguage;
  lowerRangePercent: string;
  marketType: "spot" | "usd-m" | "coin-m";
  ordinarySide: "lower" | "upper";
  referencePrice: string;
  selectedSymbol: string;
  strategyType: "ordinary_grid" | "classic_bilateral_grid";
  upperRangePercent: string;
  levels: StrategyPreviewLevel[];
};

type PreviewCandle = {
  close: number;
  close_time: number;
  high: number;
  low: number;
  open: number;
  open_time: number;
};

type PreviewSnapshot = {
  candles: PreviewCandle[];
  latest_price: string | null;
};

type ResolvedPreviewLevel = {
  direction: "buy" | "sell";
  entryPrice: string;
  entryPriceNumber: number;
  quantity: string;
  spacingPercent: string | null;
};

export function StrategyVisualPreview({
  amountMode,
  coveredRangePercent,
  generation,
  gridCount,
  lang,
  lowerRangePercent,
  marketType,
  ordinarySide,
  referencePrice,
  selectedSymbol,
  strategyType,
  upperRangePercent,
  levels,
}: Props) {
  const [siteTheme, setSiteTheme] = useState<"light" | "dark">("light");
  const [marketSnapshot, setMarketSnapshot] = useState<PreviewSnapshot | null>(null);
  const [marketLoading, setMarketLoading] = useState(false);
  const [marketError, setMarketError] = useState<string | null>(null);

  useEffect(() => {
    const root = document.documentElement;
    const applyTheme = () => {
      setSiteTheme(root.classList.contains("dark") ? "dark" : "light");
    };
    applyTheme();
    const observer = new MutationObserver(applyTheme);
    observer.observe(root, { attributeFilter: ["class"], attributes: true });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!selectedSymbol) {
      setMarketSnapshot(null);
      setMarketError(null);
      setMarketLoading(false);
      return;
    }

    let cancelled = false;
    setMarketLoading(true);
    setMarketError(null);

    fetch(`/api/market/preview?symbol=${encodeURIComponent(selectedSymbol)}&marketType=${encodeURIComponent(marketType)}`, {
      cache: "no-store",
    })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("market preview unavailable");
        }
        return (await response.json()) as PreviewSnapshot;
      })
      .then((payload) => {
        if (!cancelled) {
          setMarketSnapshot(payload);
          setMarketLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setMarketSnapshot(null);
          setMarketLoading(false);
          setMarketError(pickText(lang, "K 线加载失败，已回退到本地预览。", "Candles could not load. Falling back to local preview."));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [lang, marketType, selectedSymbol]);

  const headline = selectedSymbol || pickText(lang, "等待选择交易对", "Waiting for a symbol");
  const referenceNumber = parsePositiveNumber(referencePrice) ?? parsePositiveNumber(marketSnapshot?.latest_price ?? "");
  const latestPriceNumber = parsePositiveNumber(marketSnapshot?.latest_price ?? "") ?? referenceNumber;
  const resolvedLevels = resolvePreviewLevels(levels, strategyType, ordinarySide, referenceNumber);
  const compactLevels = resolvedLevels.slice(0, 6);
  const candles = marketSnapshot?.candles.length
    ? marketSnapshot.candles
    : buildFallbackCandles(referenceNumber, resolvedLevels);
  const chart = buildChartScene(candles, resolvedLevels, referenceNumber, latestPriceNumber, siteTheme);
  const firstLevel = resolvedLevels[0];
  const lastLevel = resolvedLevels[resolvedLevels.length - 1];
  const ordinaryLayout = strategyType === "ordinary_grid";

  return (
    <div className="space-y-4" data-strategy-preview="true">
      <Card className="overflow-hidden border-border bg-card shadow-sm">
        <CardHeader className="border-b border-border py-3">
          <CardTitle className="flex items-center gap-2 text-sm font-semibold text-foreground">
            <BarChart3 className="h-4 w-4 text-primary" />
            {pickText(lang, "图表与策略预览", "Chart & Strategy Preview")}
          </CardTitle>
        </CardHeader>
        <CardBody className="space-y-4 p-4">
          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <Metric label={pickText(lang, "交易对", "Symbol")} value={headline} />
            <Metric label={pickText(lang, "市场 / 类型", "Market / Type")} value={`${describeMarket(lang, marketType)} · ${describeStrategyType(lang, strategyType)}`} />
            <Metric label={pickText(lang, "生成 / 网格", "Generation / Grids")} value={`${describeGeneration(lang, generation)} · ${gridCount || "-"}`} />
            <Metric label={pickText(lang, ordinaryLayout ? "锚点 / 市价" : "中心 / 市价", ordinaryLayout ? "Anchor / Market" : "Center / Market")} value={`${referencePrice || "-"} · ${marketSnapshot?.latest_price ?? "-"}`} />
          </div>

          <div className="overflow-hidden rounded-3xl border border-border bg-muted/20" data-strategy-preview-chart="true">
            {selectedSymbol ? (
              <div className="space-y-3 p-2 sm:p-3">
                <svg
                  aria-label={pickText(lang, "网格 K 线预览", "Grid candle preview")}
                  className="h-[240px] sm:h-[360px] w-full"
                  preserveAspectRatio="xMidYMid meet"
                  viewBox="0 0 720 360"
                >
                  <rect fill={chart.palette.background} height="360" rx="22" width="720" x="0" y="0" />

                  {chart.rangeBand ? (
                    <rect
                      fill={chart.palette.band}
                      height={Math.max(8, chart.rangeBand.bottom - chart.rangeBand.top)}
                      opacity="0.24"
                      rx="16"
                      width="648"
                      x="48"
                      y={chart.rangeBand.top}
                    />
                  ) : null}

                  {chart.priceGuides.map((guide) => (
                    <g key={guide.label}>
                      <line
                        stroke={chart.palette.axis}
                        strokeDasharray="4 6"
                        strokeWidth="1"
                        x1="48"
                        x2="696"
                        y1={guide.y}
                        y2={guide.y}
                      />
                      <text
                        fill={chart.palette.muted}
                        fontFamily="monospace"
                        fontSize="11"
                        textAnchor="end"
                        x="42"
                        y={guide.y + 4}
                      >
                        {guide.label}
                      </text>
                    </g>
                  ))}

                  {chart.candleSeries.map((candle) => (
                    <g key={candle.key}>
                      <line
                        stroke={candle.color}
                        strokeLinecap="round"
                        strokeWidth="1.4"
                        x1={candle.x}
                        x2={candle.x}
                        y1={candle.highY}
                        y2={candle.lowY}
                      />
                      <rect
                        fill={candle.color}
                        height={candle.bodyHeight}
                        opacity="0.9"
                        rx="2"
                        width={candle.width}
                        x={candle.x - candle.width / 2}
                        y={candle.bodyY}
                      />
                    </g>
                  ))}

                  {chart.gridLines.map((gridLine) => (
                    <g key={gridLine.key}>
                      <line
                        stroke={gridLine.color}
                        strokeDasharray="3 5"
                        strokeWidth="1.4"
                        x1="48"
                        x2="696"
                        y1={gridLine.y}
                        y2={gridLine.y}
                      />
                      <text
                        fill={gridLine.color}
                        fontFamily="monospace"
                        fontSize="11"
                        textAnchor="start"
                        x="54"
                        y={gridLine.y - 4}
                      >
                        {gridLine.label}
                      </text>
                    </g>
                  ))}

                  {chart.referenceLine ? (
                    <g>
                      <line
                        stroke={chart.palette.reference}
                        strokeWidth="1.4"
                        x1="48"
                        x2="696"
                        y1={chart.referenceLine.y}
                        y2={chart.referenceLine.y}
                      />
                      <text
                        fill={chart.palette.reference}
                        fontFamily="monospace"
                        fontSize="11"
                        x="54"
                        y={chart.referenceLine.y + 14}
                      >
                        {chart.referenceLine.label}
                      </text>
                    </g>
                  ) : null}

                  {chart.latestLine ? (
                    <g>
                      <line
                        stroke={chart.palette.current}
                        strokeWidth="1.6"
                        x1="48"
                        x2="696"
                        y1={chart.latestLine.y}
                        y2={chart.latestLine.y}
                      />
                      <text
                        fill={chart.palette.current}
                        fontFamily="monospace"
                        fontSize="11"
                        textAnchor="end"
                        x="690"
                        y={chart.latestLine.y + 14}
                      >
                        {chart.latestLine.label}
                      </text>
                    </g>
                  ) : null}
                </svg>

                <div className="grid gap-2 sm:grid-cols-3">
                  <LegendPill colorClass="bg-emerald-500" label={pickText(lang, "买入 / 做多网格", "Buy / long grid")} />
                  <LegendPill colorClass="bg-orange-500" label={pickText(lang, "卖出 / 做空网格", "Sell / short grid")} />
                  <LegendPill colorClass="bg-sky-500" label={pickText(lang, "锚点 / 市价参考线", "Anchor / market guides")} />
                </div>

                {marketLoading ? (
                  <p className="text-xs text-muted-foreground">
                    {pickText(lang, "正在同步最新 K 线...", "Syncing the latest candles...")}
                  </p>
                ) : null}
                {marketError ? (
                  <p className="text-xs text-amber-600 dark:text-amber-300">{marketError}</p>
                ) : null}
              </div>
            ) : (
              <div className="flex h-[240px] sm:h-[360px] items-center justify-center px-6 text-center text-sm text-muted-foreground">
                {pickText(lang, "选择交易对后，左侧会显示锚点/中心、网格线与覆盖范围。", "Choose a symbol to render the anchor or center, grid lines, and covered range here.")}
              </div>
            )}
          </div>
        </CardBody>
      </Card>

      <div data-preview-layout={ordinaryLayout ? "ordinary" : "classic"}>
      <Card className="border-border bg-card">
        <CardHeader className="border-b border-border py-3">
          <CardTitle className="flex items-center gap-2 text-sm font-semibold text-foreground">
            <Layers3 className="h-4 w-4 text-primary" />
            {pickText(lang, ordinaryLayout ? "普通网格预览摘要" : "经典双边预览摘要", ordinaryLayout ? "Ordinary Grid Summary" : "Classic Bilateral Summary")}
          </CardTitle>
        </CardHeader>
        <CardBody className="space-y-4 p-4">
          {ordinaryLayout ? (
            <div className="grid gap-3 sm:grid-cols-4">
              <Metric dataTag="data-preview-anchor" label={pickText(lang, "锚点价格", "Anchor Price")} value={referencePrice || "-"} />
              <Metric dataTag="data-preview-range" dataTagValue="ordinary" label={pickText(lang, "覆盖范围", "Covered Range")} value={`${coveredRangePercent || "-"}%`} />
              <Metric label={pickText(lang, ordinarySide === "lower" ? "单侧方向" : "单侧方向", ordinarySide === "lower" ? "Ordinary Side" : "Ordinary Side")} value={describeOrdinarySide(lang, ordinarySide, marketType)} />
              <Metric label={pickText(lang, "当前市价", "Current Price")} value={marketSnapshot?.latest_price ?? "-"} />
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-4">
              <Metric dataTag="data-preview-center" label={pickText(lang, "中心价格", "Center Price")} value={referencePrice || "-"} />
              <Metric dataTag="data-preview-range" dataTagValue="classic-upper" label={pickText(lang, "上边范围", "Upper Range")} value={`${upperRangePercent || "-"}%`} />
              <Metric dataTag="data-preview-range" dataTagValue="classic-lower" label={pickText(lang, "下边范围", "Lower Range")} value={`${lowerRangePercent || "-"}%`} />
              <Metric label={pickText(lang, "当前市价", "Current Price")} value={marketSnapshot?.latest_price ?? "-"} />
            </div>
          )}

          {compactLevels.length > 0 ? (
            <div className="space-y-2">
              {compactLevels.map((level, index) => (
                <div
                  className="flex items-center justify-between rounded-2xl border border-border bg-background px-4 py-3"
                  key={`${level.entryPrice}-${level.quantity}-${index}`}
                >
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <div className="text-xs uppercase tracking-wide text-muted-foreground">L{index + 1}</div>
                      <span
                        className={[
                          "rounded-full px-2 py-0.5 text-[11px] font-semibold",
                          level.direction === "buy"
                            ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-300"
                            : "bg-orange-500/10 text-orange-600 dark:text-orange-300",
                        ].join(" ")}
                      >
                        {describeDirection(lang, level.direction)}
                      </span>
                    </div>
                    <div className="font-mono text-sm font-semibold text-foreground">
                      {pickText(lang, "网格价", "Grid Price")} {level.entryPrice}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {level.spacingPercent ? `${pickText(lang, "相邻间距", "Spacing")} ${level.spacingPercent}%` : pickText(lang, "首层锚定参考线", "Anchored to the reference line")}
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="font-mono text-sm text-foreground">{level.quantity}</div>
                    <div className="text-xs text-muted-foreground">
                      {amountMode === "quote" ? pickText(lang, "按 USDT 预算换算", "Derived from quote budget") : pickText(lang, "按基础币数量下单", "Uses base-asset size")}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
              {pickText(lang, "当前参数还没有生成可预览的网格。", "The current inputs have not generated any previewable levels yet.")}
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-4">
            <Metric label={pickText(lang, "首层网格", "First Grid")} value={firstLevel?.entryPrice ?? "-"} />
            <Metric label={pickText(lang, "末层网格", "Last Grid")} value={lastLevel?.entryPrice ?? "-"} />
            <Metric label={pickText(lang, "网格覆盖", "Covered Span")} value={resolveCoveredSpan(firstLevel, lastLevel)} />
            <Metric label={pickText(lang, "计量模式", "Amount Mode")} value={amountMode === "quote" ? pickText(lang, "按 USDT", "Quote Amount") : pickText(lang, "按币数量", "Base Quantity")} />
          </div>
        </CardBody>
      </Card>
    </div>

      <Card className="border-border bg-card">
        <CardBody className="grid gap-3 p-4 sm:grid-cols-3">
          <MiniInsight
            icon={<Activity className="h-4 w-4 text-primary" />}
            label={pickText(lang, "选择逻辑", "Selection Logic")}
            value={pickText(lang, "右侧一改，左侧立即重算预览，不需要先保存。", "Every change on the right recomputes the preview immediately.")}
          />
          <MiniInsight
            icon={<TrendingUp className="h-4 w-4 text-primary" />}
            label={pickText(lang, "市价参考", "Market Reference")}
            value={pickText(lang, "切到市价后，会用最新价格和 K 线重新绘制左侧预览。", "Switching to market reference redraws the preview with the latest price and candles.")}
          />
          <MiniInsight
            icon={<Layers3 className="h-4 w-4 text-primary" />}
            label={pickText(lang, "预览用途", "Preview Use")}
            value={pickText(lang, "先核对锚点/中心、覆盖范围和层级分布，再保存或启动策略。", "Review the anchor or center, covered range, and ladder structure before saving or starting.")}
          />
        </CardBody>
      </Card>
    </div>
  );
}

function Metric({
  dataTag,
  dataTagValue,
  label,
  value,
}: {
  dataTag?: string;
  dataTagValue?: string;
  label: string;
  value: string;
}) {
  const dataProps = dataTag ? { [dataTag]: dataTagValue ?? true } : {};
  return (
    <div className="rounded-2xl border border-border bg-background px-3 py-3" {...dataProps}>
      <div className="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
      <div className="mt-1 text-sm font-semibold text-foreground">{value}</div>
    </div>
  );
}

function MiniInsight({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-border bg-background px-3 py-3">
      <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
        {icon}
        {label}
      </div>
      <div className="mt-2 text-sm text-foreground">{value}</div>
    </div>
  );
}

function LegendPill({ colorClass, label }: { colorClass: string; label: string }) {
  return (
    <div className="inline-flex items-center gap-2 rounded-full border border-border bg-background px-3 py-2 text-xs text-foreground">
      <span className={`h-2.5 w-2.5 rounded-full ${colorClass}`} />
      {label}
    </div>
  );
}

function describeMarket(lang: UiLanguage, market: Props["marketType"]) {
  switch (market) {
    case "usd-m":
      return pickText(lang, "U 本位合约", "USD-M Futures");
    case "coin-m":
      return pickText(lang, "币本位合约", "COIN-M Futures");
    default:
      return pickText(lang, "现货", "Spot");
  }
}

function describeStrategyType(lang: UiLanguage, strategyType: Props["strategyType"]) {
  return strategyType === "classic_bilateral_grid"
    ? pickText(lang, "经典双边", "Classic Bilateral Grid")
    : pickText(lang, "普通网格", "Ordinary Grid");
}

function describeGeneration(lang: UiLanguage, generation: Props["generation"]) {
  switch (generation) {
    case "geometric":
      return pickText(lang, "等比", "Geometric");
    case "custom":
      return pickText(lang, "完全自定义", "Custom");
    default:
      return pickText(lang, "等差", "Arithmetic");
  }
}

function describeOrdinarySide(lang: UiLanguage, side: Props["ordinarySide"], marketType: Props["marketType"]) {
  if (side === "upper") {
    return marketType === "spot"
      ? pickText(lang, "上侧卖出", "Upper sell side")
      : pickText(lang, "上侧做空", "Upper short side");
  }
  return marketType === "spot"
    ? pickText(lang, "下侧买入", "Lower buy side")
    : pickText(lang, "下侧做多", "Lower long side");
}

function describeDirection(lang: UiLanguage, direction: "buy" | "sell") {
  return direction === "sell"
    ? pickText(lang, "卖出 / 做空", "Sell / short")
    : pickText(lang, "买入 / 做多", "Buy / long");
}

function resolvePreviewLevels(
  levels: StrategyPreviewLevel[],
  strategyType: Props["strategyType"],
  ordinarySide: Props["ordinarySide"],
  referencePrice: number | null,
) {
  return levels
    .map((level) => {
      const entryPriceNumber = parsePositiveNumber(level.entryPrice);
      if (entryPriceNumber === null) {
        return null;
      }
      return {
        direction: inferDirection(strategyType, ordinarySide, entryPriceNumber, referencePrice),
        entryPrice: level.entryPrice,
        entryPriceNumber,
        quantity: level.quantity,
        spacingPercent: level.spacingPercent,
      } satisfies ResolvedPreviewLevel;
    })
    .filter((item): item is ResolvedPreviewLevel => item !== null);
}

function inferDirection(
  strategyType: Props["strategyType"],
  ordinarySide: Props["ordinarySide"],
  entryPrice: number,
  referencePrice: number | null,
): "buy" | "sell" {
  if (strategyType === "ordinary_grid") {
    return ordinarySide === "upper" ? "sell" : "buy";
  }
  if (referencePrice !== null) {
    return entryPrice <= referencePrice ? "buy" : "sell";
  }
  return "buy";
}

function buildFallbackCandles(referencePrice: number | null, levels: ResolvedPreviewLevel[]) {
  const anchor = referencePrice ?? levels[0]?.entryPriceNumber ?? 100;
  const candles: PreviewCandle[] = [];
  const baseTime = Date.now() - 48 * 15 * 60 * 1000;
  for (let index = 0; index < 48; index += 1) {
    const swing = Math.sin(index / 3.2) * anchor * 0.012;
    const drift = (index - 24) * anchor * 0.0007;
    const open = anchor + drift + swing;
    const close = open + Math.cos(index / 2.7) * anchor * 0.006;
    const high = Math.max(open, close) + anchor * 0.0035;
    const low = Math.min(open, close) - anchor * 0.0035;
    candles.push({
      close,
      close_time: baseTime + index * 15 * 60 * 1000 + 14 * 60 * 1000,
      high,
      low,
      open,
      open_time: baseTime + index * 15 * 60 * 1000,
    });
  }
  return candles;
}

function buildChartScene(
  candles: PreviewCandle[],
  levels: ResolvedPreviewLevel[],
  referencePrice: number | null,
  latestPrice: number | null,
  siteTheme: "light" | "dark",
) {
  const palette = siteTheme === "dark"
    ? {
        axis: "#1e293b",
        background: "#07111f",
        band: "#38bdf8",
        bodyDown: "#fb7185",
        bodyUp: "#2dd4bf",
        current: "#38bdf8",
        entryBuy: "#34d399",
        entrySell: "#fb923c",
        muted: "#94a3b8",
        reference: "#facc15",
      }
    : {
        axis: "#d9e2f0",
        background: "#f8fafc",
        band: "#7dd3fc",
        bodyDown: "#ef4444",
        bodyUp: "#10b981",
        current: "#0284c7",
        entryBuy: "#059669",
        entrySell: "#ea580c",
        muted: "#64748b",
        reference: "#ca8a04",
      };
  const plot = { bottom: 320, left: 48, right: 696, top: 20 };
  const priceCandidates = [
    ...candles.flatMap((item) => [item.low, item.high]),
    ...levels.map((item) => item.entryPriceNumber),
    referencePrice ?? null,
    latestPrice ?? null,
  ].filter((item): item is number => Number.isFinite(item));
  const minPrice = priceCandidates.length > 0 ? Math.min(...priceCandidates) : 0;
  const maxPrice = priceCandidates.length > 0 ? Math.max(...priceCandidates) : 1;
  const spread = maxPrice - minPrice || Math.max(maxPrice * 0.02, 1);
  const lowerBound = minPrice - spread * 0.08;
  const upperBound = maxPrice + spread * 0.08;
  const scaleY = (price: number) => {
    const ratio = (price - lowerBound) / (upperBound - lowerBound || 1);
    return plot.bottom - ratio * (plot.bottom - plot.top);
  };
  const candleStep = candles.length > 1 ? (plot.right - plot.left) / candles.length : plot.right - plot.left;
  const candleWidth = Math.max(5, candleStep * 0.55);

  const candleSeries = candles.map((candle, index) => {
    const x = plot.left + candleStep * index + candleStep / 2;
    const openY = scaleY(candle.open);
    const closeY = scaleY(candle.close);
    return {
      bodyHeight: Math.max(3, Math.abs(closeY - openY)),
      bodyY: Math.min(openY, closeY),
      color: candle.close >= candle.open ? palette.bodyUp : palette.bodyDown,
      highY: scaleY(candle.high),
      key: String(candle.open_time),
      lowY: scaleY(candle.low),
      width: candleWidth,
      x,
    };
  });

  const gridLines = levels.map((level, index) => ({
    color: level.direction === "sell" ? palette.entrySell : palette.entryBuy,
    key: `grid-${index}-${level.entryPrice}`,
    label: `L${index + 1} ${level.entryPrice}`,
    y: scaleY(level.entryPriceNumber),
  }));

  const step = (upperBound - lowerBound) / 4;
  const priceGuides = Array.from({ length: 5 }, (_value, index) => {
    const price = lowerBound + step * index;
    return {
      label: formatDecimal(price),
      y: scaleY(price),
    };
  });

  const minLevelPrice = levels.length > 0 ? Math.min(...levels.map((level) => level.entryPriceNumber)) : null;
  const maxLevelPrice = levels.length > 0 ? Math.max(...levels.map((level) => level.entryPriceNumber)) : null;

  return {
    candleSeries,
    gridLines,
    latestLine: latestPrice !== null
      ? { label: `Last ${formatDecimal(latestPrice)}`, y: scaleY(latestPrice) }
      : null,
    palette,
    priceGuides,
    rangeBand: minLevelPrice !== null && maxLevelPrice !== null
      ? { bottom: scaleY(minLevelPrice), top: scaleY(maxLevelPrice) }
      : null,
    referenceLine: referencePrice !== null
      ? { label: `Ref ${formatDecimal(referencePrice)}`, y: scaleY(referencePrice) }
      : null,
  };
}

function parsePositiveNumber(value: string) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function formatDecimal(value: number) {
  const normalized = value.toFixed(6).replace(/\.0+$/, "").replace(/(\.\d*?)0+$/, "$1");
  return normalized === "-0" ? "0" : normalized;
}

function resolveCoveredSpan(firstLevel?: ResolvedPreviewLevel, lastLevel?: ResolvedPreviewLevel) {
  if (!firstLevel || !lastLevel) {
    return "-";
  }
  return `${firstLevel.entryPrice} - ${lastLevel.entryPrice}`;
}
