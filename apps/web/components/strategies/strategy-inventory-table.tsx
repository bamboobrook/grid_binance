"use client";

import Link from "next/link";
import { useState } from "react";
import { Pause, Play, Square, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/form";
import { StrategyStatusBadge } from "@/components/ui/strategy-status-badge";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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
  status: string;
  symbol: string;
  tradeCount: number;
  todayPnl?: string;
  runtimeDuration?: string;
  gridUtilization?: number;
};

export function StrategyInventoryTable({
  items,
  lang,
  locale,
}: {
  items: StrategyListItem[];
  lang: UiLanguage;
  locale: string;
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
      <div className="flex flex-wrap items-center gap-2">
        <Button className="h-7 border border-emerald-500/20 bg-emerald-500/10 px-3 text-[11px] font-semibold text-emerald-500 hover:bg-emerald-500/20" disabled={!hasSelection} name="intent" type="submit" value="start">
          <Play className="mr-1 h-3 w-3" />
          {pickText(lang, "批量启动", "Batch Start")}
        </Button>
        <Button className="h-7 border border-amber-500/20 bg-amber-500/10 px-3 text-[11px] font-semibold text-amber-500 hover:bg-amber-500/20" disabled={!hasSelection} name="intent" type="submit" value="pause">
          <Pause className="mr-1 h-3 w-3" />
          {pickText(lang, "批量暂停", "Batch Pause")}
        </Button>
        <Button className="h-7 border border-red-500/20 bg-red-500/10 px-3 text-[11px] font-semibold text-red-500 hover:bg-red-500/20" disabled={!hasSelection} name="intent" type="submit" value="delete">
          <Trash2 className="mr-1 h-3 w-3" />
          {pickText(lang, "批量删除", "Batch Delete")}
        </Button>
      </div>
      <div className="mt-4 overflow-x-auto rounded-xl border border-border bg-card">
        <table className="w-full min-w-[1440px] text-left text-sm whitespace-nowrap">
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
              <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "运行时长", "Uptime")}</th>
              <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "今日收益", "Today PnL")}</th>
              <th className="px-3 py-2.5 text-right font-bold">{pickText(lang, "网格利用率", "Grid Util")}</th>
              <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "动作", "Actions")}</th>
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
                    <td className="px-3 py-2 text-right text-[11px] text-muted-foreground">{strategy.runtimeDuration ?? "—"}</td>
                    <td className={`px-3 py-2 text-right font-mono text-[11px] font-bold ${pnlTone(strategy.todayPnl ?? "")}`}>{strategy.todayPnl ?? "—"}</td>
                    <td className="px-3 py-2 text-right text-[11px]">{strategy.gridUtilization != null ? `${Math.round(strategy.gridUtilization * 100)}%` : "—"}</td>
                    <td className="px-4 py-2 text-right">
                      {actions.length > 0 ? (
                        <div className="flex items-center justify-end gap-0.5 opacity-100 transition-opacity sm:opacity-0 sm:group-hover:opacity-100">
                          {actions.map((action) => (
                            <button
                              className={action.className}
                              formAction={`/api/user/strategies/${strategy.id}`}
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
                <td className="px-4 py-10 text-center text-xs text-muted-foreground" colSpan={11}>
                  {pickText(lang, "当前没有符合条件的策略，先创建你的第一个机器人。", "No active strategies yet. Create your first bot to get started.")}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </form>
  );
}

function rowActions(lang: UiLanguage, strategy: StrategyListItem) {
  const iconClass = "inline-flex h-7 w-7 items-center justify-center rounded-sm text-muted-foreground transition-colors";
  switch (strategy.status) {
    case "Running":
      return [
        {
          className: `${iconClass} hover:bg-amber-500/10 hover:text-amber-500`,
          icon: <Pause className="h-3.5 w-3.5" />,
          intent: "pause",
          title: pickText(lang, "暂停", "Pause"),
        },
        {
          className: `${iconClass} hover:bg-red-500/10 hover:text-red-500`,
          icon: <Square className="h-3.5 w-3.5" />,
          intent: "stop",
          title: pickText(lang, "停止", "Stop"),
        },
      ];
    case "Paused":
    case "ErrorPaused":
      return [
        {
          className: `${iconClass} hover:bg-emerald-500/10 hover:text-emerald-500`,
          icon: <Play className="h-3.5 w-3.5" />,
          intent: "start",
          title: pickText(lang, strategy.status === "Paused" ? "恢复" : "重新启动", strategy.status === "Paused" ? "Resume" : "Restart"),
        },
        {
          className: `${iconClass} hover:bg-red-500/10 hover:text-red-500`,
          icon: <Square className="h-3.5 w-3.5" />,
          intent: "stop",
          title: pickText(lang, "停止", "Stop"),
        },
        {
          className: `${iconClass} hover:bg-red-500/10 hover:text-red-500`,
          icon: <Trash2 className="h-3.5 w-3.5" />,
          intent: "delete",
          title: pickText(lang, "删除", "Delete"),
        },
      ];
    case "Stopping":
      return [];
    default:
      return [
        {
          className: `${iconClass} hover:bg-emerald-500/10 hover:text-emerald-500`,
          icon: <Play className="h-3.5 w-3.5" />,
          intent: "start",
          title: pickText(lang, "启动", "Start"),
        },
        {
          className: `${iconClass} hover:bg-red-500/10 hover:text-red-500`,
          icon: <Trash2 className="h-3.5 w-3.5" />,
          intent: "delete",
          title: pickText(lang, "删除", "Delete"),
        },
      ];
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

