"use client";

import Link from "next/link";
import { useState } from "react";
import { LayoutGrid, List, Pause, Play, Square, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/form";
import { StrategyStatusBadge } from "@/components/ui/strategy-status-badge";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { cn } from "@/lib/utils";

type StrategyListItem = {
  avgEntryPrice: string;
  budget: string;
  fillCount: number;
  gridCount: number;
  gridPnl: string;
  id: string;
  market: string;
  name: string;
  overallPnl: string;
  overallPnlPct?: string;
  status: string;
  symbol: string;
  tradeCount: number;
  todayPnl?: string;
  runtimeDuration?: string;
  gridUtilization?: number;
};

export function StrategyInventoryTable({
  cardViewHref,
  items,
  lang,
  locale,
  previewMode = false,
  tableViewHref,
  viewMode,
}: {
  cardViewHref: string;
  items: StrategyListItem[];
  lang: UiLanguage;
  locale: string;
  previewMode?: boolean;
  tableViewHref: string;
  viewMode: "cards" | "table";
}) {
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const hasSelection = selectedIds.length > 0;

  function toggleSelection(strategyId: string) {
    setSelectedIds((current) =>
      current.includes(strategyId) ? current.filter((value) => value !== strategyId) : [...current, strategyId],
    );
  }

  return (
    <form action="/api/user/strategies/batch" method="post">
      <input name="returnTo" type="hidden" value="list" />
      <input name="view" type="hidden" value={viewMode} />
      <div className="flex flex-wrap items-center gap-2 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            className={cn(
              "h-7 border border-emerald-500/20 bg-emerald-500/10 px-3 text-[11px] font-semibold text-emerald-500 hover:bg-emerald-500/20",
              !hasSelection && "opacity-70",
            )}
            data-batch-action="true"
            name="intent"
            type="submit"
            value="start"
          >
            <Play className="mr-1 h-3 w-3" />
            {pickText(lang, "批量启动", "Batch Start")}
          </Button>
          <Button
            className={cn(
              "h-7 border border-amber-500/20 bg-amber-500/10 px-3 text-[11px] font-semibold text-amber-500 hover:bg-amber-500/20",
              !hasSelection && "opacity-70",
            )}
            data-batch-action="true"
            name="intent"
            type="submit"
            value="pause"
          >
            <Pause className="mr-1 h-3 w-3" />
            {pickText(lang, "批量暂停", "Batch Pause")}
          </Button>
          <Button
            className={cn(
              "h-7 border border-red-500/20 bg-red-500/10 px-3 text-[11px] font-semibold text-red-500 hover:bg-red-500/20",
              !hasSelection && "opacity-70",
            )}
            data-batch-action="true"
            name="intent"
            type="submit"
            value="delete"
          >
            <Trash2 className="mr-1 h-3 w-3" />
            {pickText(lang, "批量删除", "Batch Delete")}
          </Button>
        </div>
        <div className="ml-auto inline-flex items-center rounded-lg border border-border bg-secondary p-0.5">
          <Link
            aria-pressed={viewMode === "table"}
            className={cn(
              "inline-flex h-7 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground",
              viewMode === "table" && "bg-card text-foreground shadow-sm",
            )}
            href={tableViewHref}
            title={pickText(lang, "表格视图", "Table view")}
          >
            <List className="h-4 w-4" />
          </Link>
          <Link
            aria-pressed={viewMode === "cards"}
            className={cn(
              "inline-flex h-7 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground",
              viewMode === "cards" && "bg-card text-foreground shadow-sm",
            )}
            href={cardViewHref}
            title={pickText(lang, "卡片视图", "Card view")}
          >
            <LayoutGrid className="h-4 w-4" />
          </Link>
        </div>
      </div>
      {viewMode === "table" ? (
        <div className="overflow-x-auto border-t border-border bg-card">
          <table className="w-full min-w-[1540px] text-left text-sm whitespace-nowrap">
            <thead className="border-b border-border bg-secondary/80 text-[10px] uppercase tracking-wider text-muted-foreground">
              <tr>
                <th className="w-10 px-3 py-2.5 text-center font-bold">{pickText(lang, "选择", "Pick")}</th>
                <th className="px-4 py-2.5 font-bold">{pickText(lang, "策略", "Strategy")}</th>
                <th className="px-3 py-2.5 text-center font-bold">{pickText(lang, "状态", "Status")}</th>
                <th className="px-3 py-2.5 font-bold">{pickText(lang, "市场", "Market")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "网格总数", "Grid Count")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "成交数量", "Fill Count")}</th>
                <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "平均持仓成本", "Average Cost")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "交易次数", "Trade Count")}</th>
                <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "网格盈亏", "Grid PnL")}</th>
                <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "总体盈亏", "Overall PnL")}</th>
                <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "总盈亏%", "Total PnL %")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "运行时长", "Uptime")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "今日收益", "Today PnL")}</th>
                <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "网格利用率", "Grid Util")}</th>
                <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "操作", "Operations")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/60">
              {items.length > 0 ? (
                items.map((strategy) => {
                  const actions = rowActions(lang, strategy);
                  return (
                    <tr key={strategy.id} className="group transition-colors hover:bg-secondary/60">
                      <td className="px-3 py-2 text-center">
                        <input
                          aria-label={pickText(lang, `选择策略 ${strategy.name}`, `Select strategy ${strategy.name}`)}
                          checked={selectedIds.includes(strategy.id)}
                          name="ids"
                          onChange={() => toggleSelection(strategy.id)}
                          type="checkbox"
                          className="h-3.5 w-3.5 rounded border-border bg-background accent-primary"
                          value={strategy.id}
                        />
                      </td>
                      <td className="px-4 py-2">
                        <div className="flex flex-col gap-1">
                          <Link className="text-xs font-bold text-foreground transition-colors hover:text-primary" href={`/${locale}/app/strategies/${strategy.id}`}>
                            {strategy.name}
                          </Link>
                          <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                            <span className="font-mono font-medium tracking-wide">{strategy.symbol}</span>
                            <span>{pickText(lang, "预算", "Budget")}: {strategy.budget}</span>
                          </div>
                        </div>
                      </td>
                      <td className="px-3 py-2 text-center">
                        <StrategyStatusBadge lang={lang} status={strategy.status} />
                      </td>
                      <td className="px-3 py-2">
                        <span className="rounded border border-border bg-secondary px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest text-foreground">
                          {describeMarket(lang, strategy.market)}
                        </span>
                      </td>
                      <td className="px-3 py-2 text-right font-mono text-[11px] font-bold text-foreground">{strategy.gridCount}</td>
                      <td className="px-3 py-2 text-right font-mono text-[11px] font-bold text-foreground">{strategy.fillCount}</td>
                      <td className="px-4 py-2 text-right font-mono text-[11px] font-bold text-foreground">{strategy.avgEntryPrice}</td>
                      <td className="px-3 py-2 text-right font-mono text-[11px] font-bold text-foreground">{strategy.tradeCount}</td>
                      <td className={`px-4 py-2 text-right font-mono text-[11px] font-bold ${pnlTone(strategy.gridPnl)}`}>{strategy.gridPnl}</td>
                      <td className={`px-4 py-2 text-right font-mono text-[11px] font-bold ${pnlTone(strategy.overallPnl)}`}>{strategy.overallPnl}</td>
                      <td className={`px-4 py-2 text-right font-mono text-[11px] font-bold ${pnlTone(strategy.overallPnlPct ?? "")}`}>{strategy.overallPnlPct ?? "—"}</td>
                      <td className="px-3 py-2 text-right text-[11px] text-muted-foreground">{strategy.runtimeDuration ?? "—"}</td>
                      <td className={`px-3 py-2 text-right font-mono text-[11px] font-bold ${pnlTone(strategy.todayPnl ?? "")}`}>{strategy.todayPnl ?? "—"}</td>
                      <td className="px-3 py-2 text-right text-[11px]">{strategy.gridUtilization != null ? `${Math.round(strategy.gridUtilization * 100)}%` : "—"}</td>
                      <td className="px-4 py-2 text-right">
                        {actions.length > 0 ? (
                          <div className="flex items-center justify-end gap-1">
                            {actions.map((action) => (
                              <button
                                className={action.className}
                                aria-label={`${action.title}: ${strategy.name}`}
                                formAction={`/api/user/strategies/${strategy.id}?view=${viewMode}`}
                                formMethod="post"
                                key={`${strategy.id}-${action.intent}`}
                                name="intent"
                                title={action.title}
                                type="submit"
                                value={action.intent}
                              >
                                {action.icon}
                              </button>
                            ))}
                          </div>
                        ) : (
                          <span className="text-[10px] font-semibold text-muted-foreground">{pickText(lang, "停止处理中", "Stopping")}</span>
                        )}
                      </td>
                    </tr>
                  );
                })
              ) : (
                <tr>
                  <td className="px-4 py-10 text-center text-xs text-muted-foreground" colSpan={15}>
                    {pickText(lang, "当前没有符合条件的策略，先创建你的第一个机器人。", "No active strategies yet. Create your first bot to get started.")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="grid gap-3 border-t border-border bg-card p-3 md:grid-cols-2 xl:grid-cols-3">
          {items.length > 0 ? (
            items.map((strategy) => {
              const actions = rowActions(lang, strategy);
              return (
                <div key={strategy.id} className="flex min-h-[18.5rem] rounded-md border border-border bg-background p-4">
                  <div className="flex min-h-full w-full items-stretch gap-3">
                    <input
                      aria-label={pickText(lang, `选择策略 ${strategy.name}`, `Select strategy ${strategy.name}`)}
                      checked={selectedIds.includes(strategy.id)}
                      className="mt-1 h-3.5 w-3.5 rounded border-border bg-background accent-primary"
                      name="ids"
                      onChange={() => toggleSelection(strategy.id)}
                      type="checkbox"
                      value={strategy.id}
                    />
                    <div className="flex min-w-0 flex-1 flex-col">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <Link className="block truncate text-sm font-bold text-foreground transition-colors hover:text-primary" href={`/${locale}/app/strategies/${strategy.id}`}>
                            {strategy.name}
                          </Link>
                          <p className="mt-1 text-[11px] text-muted-foreground">
                            <span className="font-mono">{strategy.symbol}</span>
                            <span className="mx-1.5">/</span>
                            {strategy.budget}
                          </p>
                        </div>
                        <StrategyStatusBadge lang={lang} status={strategy.status} />
                      </div>
                      <div className="mt-4 grid grid-cols-2 gap-3 text-xs">
                        <Metric label={pickText(lang, "总体盈亏", "Overall PnL")} tone={pnlTone(strategy.overallPnl)} value={strategy.overallPnl} />
                        <Metric label={pickText(lang, "总盈亏%", "Total PnL %")} tone={pnlTone(strategy.overallPnlPct ?? "")} value={strategy.overallPnlPct ?? "—"} />
                        <Metric label={pickText(lang, "今日收益", "Today PnL")} tone={pnlTone(strategy.todayPnl ?? "")} value={strategy.todayPnl ?? "—"} />
                        <Metric label={pickText(lang, "网格利用率", "Grid Util")} value={strategy.gridUtilization != null ? `${Math.round(strategy.gridUtilization * 100)}%` : "—"} />
                        <Metric label={pickText(lang, "网格 / 成交", "Grid / Fills")} value={`${strategy.gridCount} / ${strategy.fillCount}`} />
                        <Metric label={pickText(lang, "均价", "Average Cost")} value={strategy.avgEntryPrice} />
                      </div>
                      <div className="mt-auto pt-4 text-[11px] text-muted-foreground">
                        <span className="block">{describeMarket(lang, strategy.market)}</span>
                        <div className="mt-2 flex flex-wrap items-center justify-end gap-1.5">
                          {actions.length > 0 ? (
                            actions.map((action) => (
                              <button
                                aria-label={`${action.title}: ${strategy.name}`}
                                className={action.cardClassName}
                                formAction={`/api/user/strategies/${strategy.id}?view=${viewMode}`}
                                formMethod="post"
                                key={`${strategy.id}-${action.intent}`}
                                name="intent"
                                title={action.title}
                                type="submit"
                                value={action.intent}
                              >
                                {action.icon}
                                <span>{action.label}</span>
                              </button>
                            ))
                          ) : (
                            <span className="text-[10px] font-semibold text-muted-foreground">{pickText(lang, "停止处理中", "Stopping")}</span>
                          )}
                          <Link className="inline-flex h-7 items-center justify-center rounded-sm border border-border bg-card px-2 text-[11px] font-semibold text-foreground transition-colors hover:border-primary/50 hover:text-primary" href={`/${locale}/app/strategies/${strategy.id}`}>
                            {pickText(lang, "详情", "Details")}
                          </Link>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              );
            })
          ) : (
            <div className="col-span-full px-4 py-10 text-center text-xs text-muted-foreground">
              {pickText(lang, "当前没有符合条件的策略，先创建你的第一个机器人。", "No active strategies yet. Create your first bot to get started.")}
            </div>
          )}
        </div>
      )}
    </form>
  );
}

function Metric({ label, tone, value }: { label: string; tone?: string; value: string }) {
  return (
    <div className="rounded-sm border border-border/70 bg-card px-3 py-2">
      <p className="text-[10px] font-semibold text-muted-foreground">{label}</p>
      <p className={cn("mt-1 font-mono text-sm font-bold text-foreground", tone)}>{value}</p>
    </div>
  );
}

function rowActions(lang: UiLanguage, strategy: StrategyListItem) {
  switch (strategy.status) {
    case "Running":
      return [
        {
          cardClassName: cardActionClass("pause"),
          className: tableActionClass("pause"),
          icon: <Pause className="h-3.5 w-3.5" />,
          intent: "pause",
          label: pickText(lang, "暂停", "Pause"),
          title: pickText(lang, "暂停", "Pause"),
        },
        {
          cardClassName: cardActionClass("danger"),
          className: tableActionClass("danger"),
          icon: <Square className="h-3.5 w-3.5" />,
          intent: "stop",
          label: pickText(lang, "停止", "Stop"),
          title: pickText(lang, "停止", "Stop"),
        },
      ];
    case "Paused":
    case "ErrorPaused":
      return [
        {
          cardClassName: cardActionClass("start"),
          className: tableActionClass("start"),
          icon: <Play className="h-3.5 w-3.5" />,
          intent: "start",
          label: pickText(lang, strategy.status === "Paused" ? "恢复" : "重启", strategy.status === "Paused" ? "Resume" : "Restart"),
          title: pickText(lang, strategy.status === "Paused" ? "恢复" : "重新启动", strategy.status === "Paused" ? "Resume" : "Restart"),
        },
        {
          cardClassName: cardActionClass("danger"),
          className: tableActionClass("danger"),
          icon: <Square className="h-3.5 w-3.5" />,
          intent: "stop",
          label: pickText(lang, "停止", "Stop"),
          title: pickText(lang, "停止", "Stop"),
        },
        {
          cardClassName: cardActionClass("danger"),
          className: tableActionClass("danger"),
          icon: <Trash2 className="h-3.5 w-3.5" />,
          intent: "delete",
          label: pickText(lang, "删除", "Delete"),
          title: pickText(lang, "删除", "Delete"),
        },
      ];
    case "Stopping":
      return [];
    default:
      return [
        {
          cardClassName: cardActionClass("start"),
          className: tableActionClass("start"),
          icon: <Play className="h-3.5 w-3.5" />,
          intent: "start",
          label: pickText(lang, "启动", "Start"),
          title: pickText(lang, "启动", "Start"),
        },
        {
          cardClassName: cardActionClass("danger"),
          className: tableActionClass("danger"),
          icon: <Trash2 className="h-3.5 w-3.5" />,
          intent: "delete",
          label: pickText(lang, "删除", "Delete"),
          title: pickText(lang, "删除", "Delete"),
        },
      ];
  }
}

function tableActionClass(tone: "danger" | "pause" | "start") {
  return cn(
    "inline-flex h-7 w-7 items-center justify-center rounded-sm border text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
    actionToneClass(tone),
  );
}

function cardActionClass(tone: "danger" | "pause" | "start") {
  return cn(
    "inline-flex h-7 items-center justify-center gap-1.5 rounded-sm border px-2 text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
    actionToneClass(tone),
  );
}

function actionToneClass(tone: "danger" | "pause" | "start") {
  switch (tone) {
    case "start":
      return "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 hover:bg-emerald-500/20 dark:text-emerald-300";
    case "pause":
      return "border-amber-500/40 bg-amber-500/10 text-amber-700 hover:bg-amber-500/20 dark:text-amber-300";
    case "danger":
      return "border-red-500/40 bg-red-500/10 text-red-700 hover:bg-red-500/20 dark:text-red-300";
  }
}

function describeMarket(lang: UiLanguage, market: string) {
  switch (market) {
    case "Spot":
      return pickText(lang, "现货", "Spot");
    case "FuturesUsdM":
      return pickText(lang, "U本位合约", "USD-M Futures");
    case "FuturesCoinM":
      return pickText(lang, "币本位合约", "COIN-M Futures");
    default:
      return market;
  }
}

function pnlTone(value: string) {
  const numeric = Number.parseFloat(value);
  if (!Number.isFinite(numeric)) {
    return "text-foreground";
  }
  if (numeric > 0) {
    return "text-emerald-500";
  }
  if (numeric < 0) {
    return "text-red-500";
  }
  return "text-foreground";
}
