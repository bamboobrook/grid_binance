import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Filter, LayoutGrid, List, Pause, Play, Plus, Search, Trash2 } from "lucide-react";

import { Button, Select } from "@/components/ui/form";
import { Card } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ notice?: string | string[]; error?: string | string[]; status?: string | string[]; symbol?: string | string[] }>;
};

type StrategyListResponse = {
  items: Array<{
    budget: string;
    id: string;
    market: string;
    name: string;
    status: string;
    symbol: string;
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function copy(locale: string, zh: string, en: string) {
  return locale.startsWith("zh") ? zh : en;
}

function formatNotice(locale: string, notice: string) {
  switch (notice) {
    case "batch-start-complete":
      return copy(locale, "批量启动完成", "Batch start completed");
    case "batch-pause-complete":
      return copy(locale, "批量暂停完成", "Batch pause completed");
    case "batch-delete-complete":
      return copy(locale, "批量删除完成", "Batch delete completed");
    case "stop-all-complete":
      return copy(locale, "全部停止完成", "Stop-all completed");
    case "strategy-deleted":
      return copy(locale, "策略已删除", "Strategy deleted");
    default:
      return notice;
  }
}

export default async function StrategiesPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "strategies" });

  const searchParamsValue = (await searchParams) ?? {};
  const notice = firstValue(searchParamsValue.notice);
  const error = firstValue(searchParamsValue.error);
  const statusFilter = firstValue(searchParamsValue.status) ?? "all";
  const symbolFilter = firstValue(searchParamsValue.symbol) ?? "";

  const strategyResult = await fetchStrategies();
  const strategies = strategyResult.items;
  const filteredStrategies = strategies.filter((item) => {
    const statusMatches = statusFilter === "all" || item.status === statusFilter;
    const query = symbolFilter.trim().toLowerCase();
    const symbolMatches = !query || item.symbol.toLowerCase().includes(query) || item.name.toLowerCase().includes(query);
    return statusMatches && symbolMatches;
  });

  return (
    <div className="flex flex-col space-y-4 max-w-[1600px] mx-auto h-full">
      {notice ? <StatusBanner title={formatNotice(locale, notice)} description={copy(locale, "本次操作已经写入后端，你可以继续筛选或进入详情页核对。", "The latest action has been persisted and you can keep filtering or open the detail workspace.")} /> : null}
      {error ? <StatusBanner title={copy(locale, "策略操作失败", "Strategy action failed")} description={error} /> : null}
      {strategyResult.error ? <StatusBanner title={copy(locale, "策略列表暂不可用", "Strategy catalog unavailable")} description={strategyResult.error} /> : null}

      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-slate-100">{t("title")}</h1>
          <p className="text-sm text-muted-foreground">{copy(locale, "支持按筛选结果批量启动、暂停、删除，并保留全部停止。", "Batch start, pause, delete, and global stop-all all post to the live backend.")}</p>
        </div>
        <div className="flex items-center gap-3">
          <form action="/api/user/strategies/batch" method="post">
            <input name="intent" type="hidden" value="stop-all" />
            <Button className="h-8 px-3 text-xs bg-red-500/10 text-red-500 hover:bg-red-500/20 border border-red-500/20">
              <Pause className="w-3.5 h-3.5 mr-1.5" />
              {t("stopAll")}
            </Button>
          </form>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              {t("new")}
            </Button>
          </Link>
        </div>
      </div>

      <form method="get" className="bg-card border border-border/60 rounded-sm p-3 flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-input rounded-sm border border-border focus-within:border-primary/50 transition-colors flex-1 min-w-[220px] max-w-[360px]">
          <Search className="w-4 h-4 text-muted-foreground" />
          <input
            name="symbol"
            type="text"
            placeholder={t("filter")}
            defaultValue={symbolFilter}
            className="bg-transparent border-none outline-none text-xs w-full text-foreground placeholder:text-muted-foreground"
          />
        </div>
        <Select defaultValue={statusFilter} name="status" className="min-w-[160px] bg-input border-border text-xs">
          <option value="all">{copy(locale, "全部状态", "All statuses")}</option>
          <option value="Draft">{copy(locale, "草稿", "Draft")}</option>
          <option value="Running">{copy(locale, "运行中", "Running")}</option>
          <option value="Paused">{copy(locale, "已暂停", "Paused")}</option>
          <option value="ErrorPaused">{copy(locale, "异常暂停", "Error paused")}</option>
          <option value="Stopped">{copy(locale, "已停止", "Stopped")}</option>
        </Select>
        <Button type="submit" className="h-8 px-3 text-xs">
          <Filter className="w-3.5 h-3.5 mr-1.5" />
          {copy(locale, "应用筛选", "Apply filter")}
        </Button>
        <div className="flex items-center gap-1 ml-auto bg-input p-1 rounded-sm border border-border">
          <button type="button" className="p-1.5 bg-secondary text-foreground rounded-sm"><List className="w-4 h-4" /></button>
          <button type="button" className="p-1.5 text-muted-foreground hover:text-foreground rounded-sm transition-colors"><LayoutGrid className="w-4 h-4" /></button>
        </div>
      </form>

      <form action="/api/user/strategies/batch" method="post" className="flex flex-wrap items-center gap-2">
        {filteredStrategies.map((strategy) => (
          <input key={strategy.id} name="ids" type="hidden" value={strategy.id} />
        ))}
        <Button name="intent" type="submit" value="start" className="h-8 px-3 text-xs" disabled={filteredStrategies.length === 0}>
          <Play className="w-3.5 h-3.5 mr-1.5" />
          {copy(locale, "按筛选批量启动", "Start filtered")}
        </Button>
        <Button name="intent" type="submit" value="pause" className="h-8 px-3 text-xs" disabled={filteredStrategies.length === 0}>
          <Pause className="w-3.5 h-3.5 mr-1.5" />
          {copy(locale, "按筛选批量暂停", "Pause filtered")}
        </Button>
        <Button name="intent" type="submit" value="delete" className="h-8 px-3 text-xs" disabled={filteredStrategies.length === 0}>
          <Trash2 className="w-3.5 h-3.5 mr-1.5" />
          {copy(locale, "按筛选批量删除", "Delete filtered")}
        </Button>
        <span className="text-xs text-muted-foreground">{copy(locale, `当前筛中 ${filteredStrategies.length} 个策略`, `${filteredStrategies.length} strategies currently match the filter`)}</span>
      </form>

      <Card className="bg-card border-border shadow-none">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="bg-muted text-muted-foreground text-[10px] uppercase tracking-wider">
              <tr>
                <th className="px-4 py-2 font-medium">{t("table.strategy")}</th>
                <th className="px-4 py-2 font-medium">{t("table.market")}</th>
                <th className="px-4 py-2 font-medium text-center">{t("table.status")}</th>
                <th className="px-4 py-2 font-medium text-right">{t("table.exposure")}</th>
                <th className="px-4 py-2 font-medium text-right">{t("table.actions")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/50">
              {filteredStrategies.length > 0 ? filteredStrategies.map((strategy) => (
                <tr key={strategy.id} className="hover:bg-secondary/30 transition-colors group">
                  <td className="px-4 py-3">
                    <div className="flex flex-col gap-0.5">
                      <Link href={`/${locale}/app/strategies/${strategy.id}`} className="text-sm font-bold text-foreground hover:text-primary transition-colors">
                        {strategy.name}
                      </Link>
                      <span className="text-[10px] text-muted-foreground font-mono tracking-wide">{strategy.symbol}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="px-1.5 py-0.5 bg-secondary border border-border text-foreground rounded-[2px] text-[10px] font-bold uppercase tracking-widest">
                      {strategy.market}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <StatusBadge locale={locale} status={strategy.status} />
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs text-foreground font-semibold">
                    ${strategy.budget}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2 opacity-100 transition-opacity">
                      <form action={`/api/user/strategies/${strategy.id}`} method="post" className="inline-flex items-center gap-1">
                        <Button size="icon" name="intent" type="submit" value={strategy.status === "Running" ? "pause" : "start"} className="h-7 w-7 text-muted-foreground hover:text-emerald-500 hover:bg-emerald-500/10">
                          {strategy.status === "Running" ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
                        </Button>
                        <Button size="icon" name="intent" type="submit" value="delete" className="h-7 w-7 text-muted-foreground hover:text-red-500 hover:bg-red-500/10">
                          <Trash2 className="w-3.5 h-3.5" />
                        </Button>
                      </form>
                      <Link href={`/${locale}/app/strategies/${strategy.id}`} className="text-[11px] text-primary hover:underline">
                        {copy(locale, "详情", "Details")}
                      </Link>
                    </div>
                  </td>
                </tr>
              )) : (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-xs text-muted-foreground">
                    {copy(locale, "当前没有符合筛选条件的策略。", "No strategies match the current filter.")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}

function StatusBadge({ status, locale }: { status: string; locale: string }) {
  const isRunning = status === "Running";
  const isPaused = status === "Paused";
  const isDraft = status === "Draft";

  return (
    <span className={`inline-flex items-center px-1.5 py-0.5 rounded-[2px] text-[10px] font-bold uppercase tracking-widest ${
      isRunning ? "bg-emerald-500/10 text-emerald-500" :
      isPaused ? "bg-amber-500/10 text-amber-500" :
      isDraft ? "bg-blue-500/10 text-blue-500" :
      "bg-secondary text-muted-foreground"
    }`}>
      {status === "Running" ? copy(locale, "运行中", "Running") : status === "Paused" ? copy(locale, "已暂停", "Paused") : status === "Draft" ? copy(locale, "草稿", "Draft") : status === "ErrorPaused" ? copy(locale, "异常暂停", "Error paused") : status === "Stopped" ? copy(locale, "已停止", "Stopped") : status}
    </span>
  );
}

async function fetchStrategies(): Promise<{ items: StrategyListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { items: [], error: null };
  }
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: [], error: "Strategy catalog is temporarily unavailable." };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
