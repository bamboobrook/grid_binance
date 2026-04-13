"use client";

import Link from "next/link";
import { useState } from "react";
import { Pause, Play, Square, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type StrategyListItem = {
  budget: string;
  id: string;
  market: string;
  name: string;
  status: string;
  symbol: string;
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
      <div className="flex flex-wrap gap-2">
        <Button className="h-8 px-3 text-xs" disabled={!hasSelection} name="intent" type="submit" value="start">
          <Play className="mr-1.5 h-3.5 w-3.5" />
          {pickText(lang, "批量启动", "Batch Start")}
        </Button>
        <Button className="h-8 px-3 text-xs" disabled={!hasSelection} name="intent" type="submit" value="pause">
          <Pause className="mr-1.5 h-3.5 w-3.5" />
          {pickText(lang, "批量暂停", "Batch Pause")}
        </Button>
        <Button className="h-8 px-3 text-xs" disabled={!hasSelection} name="intent" type="submit" value="delete">
          <Trash2 className="mr-1.5 h-3.5 w-3.5" />
          {pickText(lang, "批量删除", "Batch Delete")}
        </Button>
      </div>

      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead className="bg-muted text-[10px] uppercase tracking-wider text-muted-foreground">
            <tr>
              <th className="w-12 px-4 py-2 text-center font-medium">{pickText(lang, "选择", "Pick")}</th>
              <th className="px-4 py-2 font-medium">{pickText(lang, "策略", "Strategy")}</th>
              <th className="px-4 py-2 font-medium">{pickText(lang, "市场", "Market")}</th>
              <th className="px-4 py-2 text-center font-medium">{pickText(lang, "状态", "Status")}</th>
              <th className="px-4 py-2 text-right font-medium">{pickText(lang, "敞口", "Exposure")}</th>
              <th className="px-4 py-2 text-right font-medium">{pickText(lang, "动作", "Actions")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/50">
            {items.length > 0 ? (
              items.map((strategy) => (
                <tr key={strategy.id} className="group transition-colors hover:bg-secondary/30">
                  <td className="px-4 py-3 text-center">
                    <input
                      aria-label={pickText(lang, `选择策略 ${strategy.name}`, `Select strategy ${strategy.name}`)}
                      checked={selectedIds.includes(strategy.id)}
                      name="ids"
                      onChange={() => toggleSelection(strategy.id)}
                      type="checkbox"
                      value={strategy.id}
                    />
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-col gap-0.5">
                      <Link className="text-sm font-bold text-foreground transition-colors hover:text-primary" href={`/${locale}/app/strategies/${strategy.id}`}>
                        {strategy.name}
                      </Link>
                      <span className="font-mono text-[10px] tracking-wide text-muted-foreground">{strategy.symbol}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="rounded-[2px] border border-border bg-secondary px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest text-foreground">
                      {describeMarket(lang, strategy.market)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <StatusBadge lang={lang} status={strategy.status} />
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs font-semibold text-foreground">
                    ${strategy.budget}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1 opacity-100 transition-opacity sm:opacity-0 sm:group-hover:opacity-100">
                      {rowActions(lang, strategy).map((action) => (
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
                  </td>
                </tr>
              ))
            ) : (
              <tr>
                <td className="px-4 py-12 text-center text-xs text-muted-foreground" colSpan={6}>
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
          title: pickText(lang, "恢复", "Resume"),
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

function StatusBadge({ lang, status }: { lang: UiLanguage; status: string }) {
  const label =
    status === "Running" ? pickText(lang, "运行中", "Running") :
    status === "Paused" ? pickText(lang, "已暂停", "Paused") :
    status === "Draft" ? pickText(lang, "草稿", "Draft") :
    status === "ErrorPaused" ? pickText(lang, "异常阻塞", "Blocked") :
    status === "Stopped" ? pickText(lang, "已停止", "Stopped") :
    status === "Completed" ? pickText(lang, "已完成", "Completed") :
    status;

  return (
    <span className={`inline-flex items-center rounded-[2px] px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest ${
      status === "Running" ? "bg-emerald-500/10 text-emerald-500" :
      status === "Paused" ? "bg-amber-500/10 text-amber-500" :
      status === "Draft" ? "bg-blue-500/10 text-blue-500" :
      status === "ErrorPaused" ? "bg-red-500/10 text-red-500" :
      "bg-secondary text-muted-foreground"
    }`}>
      {label}
    </span>
  );
}
