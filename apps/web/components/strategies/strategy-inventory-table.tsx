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
      <div className="flex flex-wrap items-center gap-2">
        <Button className="h-7 px-3 text-[11px] font-semibold bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 border border-emerald-500/20 transition-colors" disabled={!hasSelection} name="intent" type="submit" value="start">
          <Play className="mr-1 h-3 w-3" />
          {pickText(lang, "批量启动", "Batch Start")}
        </Button>
        <Button className="h-7 px-3 text-[11px] font-semibold bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 border border-amber-500/20 transition-colors" disabled={!hasSelection} name="intent" type="submit" value="pause">
          <Pause className="mr-1 h-3 w-3" />
          {pickText(lang, "批量暂停", "Batch Pause")}
        </Button>
        <Button className="h-7 px-3 text-[11px] font-semibold bg-red-500/10 text-red-400 hover:bg-red-500/20 border border-red-500/20 transition-colors" disabled={!hasSelection} name="intent" type="submit" value="delete">
          <Trash2 className="mr-1 h-3 w-3" />
          {pickText(lang, "批量删除", "Batch Delete")}
        </Button>
      </div>
      <div className="mt-4 rounded-xl border border-slate-800 overflow-hidden bg-[#111827]">
        <table className="w-full text-left text-sm whitespace-nowrap">
          <thead className="bg-[#0f141f] text-[10px] uppercase tracking-wider text-slate-500 border-b border-slate-800">
            <tr>
              <th className="w-10 px-3 py-2.5 text-center font-bold">{pickText(lang, "选择", "Pick")}</th>
              <th className="px-4 py-2.5 font-bold">{pickText(lang, "策略", "Strategy")}</th>
              <th className="px-3 py-2.5 font-bold">{pickText(lang, "市场", "Market")}</th>
              <th className="px-3 py-2.5 text-center font-bold">{pickText(lang, "状态", "Status")}</th>
              <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "敞口 (USDT)", "Exposure (USDT)")}</th>
              <th className="px-4 py-2.5 text-right font-bold">{pickText(lang, "动作", "Actions")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/50">
            {items.length > 0 ? (
              items.map((strategy) => (
                <tr key={strategy.id} className="group transition-colors hover:bg-[#1f2937]/50">
                  <td className="px-3 py-1.5 text-center">
                    <input
                      aria-label={pickText(lang, `选择策略 ${strategy.name}`, `Select strategy ${strategy.name}`)}
                      checked={selectedIds.includes(strategy.id)}
                      name="ids"
                      onChange={() => toggleSelection(strategy.id)}
                      type="checkbox"
                      className="accent-primary w-3.5 h-3.5 bg-slate-800 border-slate-600 rounded"
                      value={strategy.id}
                    />
                  </td>
                  <td className="px-4 py-1.5">
                    <div className="flex flex-col">
                      <Link className="text-xs font-bold text-slate-200 transition-colors hover:text-primary" href={`/${locale}/app/strategies/${strategy.id}`}>
                        {strategy.name}
                      </Link>
                      <span className="font-mono text-[9px] font-medium tracking-wide text-slate-500">{strategy.symbol}</span>
                    </div>
                  </td>
                  <td className="px-3 py-1.5">
                    <span className="rounded border border-slate-700 bg-slate-800 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest text-slate-300">
                      {describeMarket(lang, strategy.market)}
                    </span>
                  </td>
                  <td className="px-3 py-1.5 text-center">
                    <StatusBadge lang={lang} status={strategy.status} />
                  </td>
                  <td className="px-4 py-1.5 text-right font-mono text-[11px] font-bold text-white">
                    ${strategy.budget}
                  </td>
                  <td className="px-4 py-1.5 text-right">
                    <div className="flex items-center justify-end gap-0.5 opacity-100 transition-opacity sm:opacity-0 sm:group-hover:opacity-100">
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
                <td className="px-4 py-10 text-center text-xs text-slate-500" colSpan={6}>
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
    <span className={`inline-flex items-center rounded border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest ${
      status === "Running" ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20" :
      status === "Paused" ? "bg-amber-500/10 text-amber-400 border-amber-500/20" :
      status === "Draft" ? "bg-blue-500/10 text-blue-400 border-blue-500/20" :
      status === "ErrorPaused" ? "bg-red-500/10 text-red-400 border-red-500/20" :
      "bg-slate-800 text-slate-400 border-slate-700"
    }`}>
      {label}
    </span>
  );
}
