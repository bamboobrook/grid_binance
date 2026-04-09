import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Filter, LayoutGrid, List, Pause, Play, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/form";
import { Card } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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

export default async function StrategiesPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const t = await getTranslations({ locale, namespace: "strategies" });

  const searchParamsValue = (await searchParams) ?? {};
  const statusFilter = firstValue(searchParamsValue.status) ?? "all";
  const symbolFilter = firstValue(searchParamsValue.symbol) ?? "";

  const strategyResult = await fetchStrategies(lang);
  const strategies = strategyResult.items;
  const filteredStrategies = strategies.filter((item) => {
    const statusMatches = statusFilter === "all" || item.status === statusFilter;
    const query = symbolFilter.trim().toLowerCase();
    const symbolMatches = !query || item.symbol.toLowerCase().includes(query) || item.name.toLowerCase().includes(query);
    return statusMatches && symbolMatches;
  });

  return (
    <div className="flex flex-col space-y-4 max-w-[1600px] mx-auto h-full">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-foreground">{t("title")}</h1>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <form action="/api/user/strategies/batch" method="post">
            <input name="intent" type="hidden" value="stop-all" />
            <Button className="h-8 px-3 text-xs bg-red-500/10 text-red-500 hover:bg-red-500/20 border border-red-500/20">
              <Pause className="w-3.5 h-3.5 mr-1.5" />
              {pickText(lang, "全部停止", "Stop All")}
            </Button>
          </form>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              {pickText(lang, "新建机器人", "New Bot")}
            </Button>
          </Link>
        </div>
      </div>

      <form action={`/${locale}/app/strategies`} method="get" className="bg-card border border-border/60 rounded-xl p-3 flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-input rounded-xl border border-border focus-within:border-primary/50 transition-colors flex-1 max-w-[320px]">
          <Filter className="w-4 h-4 text-muted-foreground" />
          <input
            type="text"
            name="symbol"
            placeholder={t("filter")}
            defaultValue={symbolFilter}
            className="bg-transparent border-none outline-none text-xs w-full text-foreground placeholder:text-muted-foreground"
          />
        </div>
        <select defaultValue={statusFilter} name="status" className="h-9 rounded-xl border border-border bg-input px-3 text-xs text-foreground">
          <option value="all">{pickText(lang, "全部状态", "All Status")}</option>
          <option value="Draft">{pickText(lang, "草稿", "Draft")}</option>
          <option value="Running">{pickText(lang, "运行中", "Running")}</option>
          <option value="Paused">{pickText(lang, "已暂停", "Paused")}</option>
          <option value="ErrorPaused">{pickText(lang, "异常阻塞", "Blocked")}</option>
        </select>
        <Button className="h-9 px-4 text-xs" type="submit">{pickText(lang, "应用筛选", "Apply Filters")}</Button>
        <div className="flex items-center gap-1 ml-auto bg-input p-1 rounded-xl border border-border">
          <button className="p-1.5 bg-secondary text-foreground rounded-lg" type="button">
            <List className="w-4 h-4" />
          </button>
          <button className="p-1.5 text-muted-foreground hover:text-foreground rounded-lg transition-colors" type="button">
            <LayoutGrid className="w-4 h-4" />
          </button>
        </div>
      </form>

      <form action="/api/user/strategies/batch" method="post">
        {filteredStrategies.map((strategy) => (
          <input key={strategy.id} name="ids" type="hidden" value={strategy.id} />
        ))}
        <div className="flex flex-wrap gap-2">
          <Button className="h-8 px-3 text-xs" name="intent" type="submit" value="start">
            <Play className="w-3.5 h-3.5 mr-1.5" />
            {pickText(lang, "批量启动", "Batch Start")}
          </Button>
          <Button className="h-8 px-3 text-xs" name="intent" type="submit" value="pause">
            <Pause className="w-3.5 h-3.5 mr-1.5" />
            {pickText(lang, "批量暂停", "Batch Pause")}
          </Button>
          <Button className="h-8 px-3 text-xs" name="intent" type="submit" value="delete">
            <Trash2 className="w-3.5 h-3.5 mr-1.5" />
            {pickText(lang, "批量删除", "Batch Delete")}
          </Button>
        </div>
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
                      {describeMarket(lang, strategy.market)}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <StatusBadge lang={lang} status={strategy.status} />
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs text-foreground font-semibold">
                    ${strategy.budget}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1 opacity-100 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity">
                      <form action={`/api/user/strategies/${strategy.id}`} method="post">
                        <Button size="icon" className="h-7 w-7 text-muted-foreground hover:text-emerald-500 hover:bg-emerald-500/10" name="intent" title={pickText(lang, "启动", "Start")} type="submit" value="start">
                          <Play className="w-3.5 h-3.5" />
                        </Button>
                      </form>
                      <form action={`/api/user/strategies/${strategy.id}`} method="post">
                        <Button size="icon" className="h-7 w-7 text-muted-foreground hover:text-amber-500 hover:bg-amber-500/10" name="intent" title={pickText(lang, "暂停", "Pause")} type="submit" value="pause">
                          <Pause className="w-3.5 h-3.5" />
                        </Button>
                      </form>
                      <form action={`/api/user/strategies/${strategy.id}`} method="post">
                        <Button size="icon" className="h-7 w-7 text-muted-foreground hover:text-red-500 hover:bg-red-500/10" name="intent" title={pickText(lang, "删除", "Delete")} type="submit" value="delete">
                          <Trash2 className="w-3.5 h-3.5" />
                        </Button>
                      </form>
                    </div>
                  </td>
                </tr>
              )) : (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-xs text-muted-foreground">
                    {pickText(lang, "当前没有符合条件的策略，先创建你的第一个机器人。", "No active strategies yet. Create your first bot to get started.")}
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

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
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
  const isRunning = status === "Running";
  const isPaused = status === "Paused";
  const isDraft = status === "Draft";
  const isBlocked = status === "ErrorPaused";
  const label =
    status === "Running" ? pickText(lang, "运行中", "Running") :
    status === "Paused" ? pickText(lang, "已暂停", "Paused") :
    status === "Draft" ? pickText(lang, "草稿", "Draft") :
    status === "ErrorPaused" ? pickText(lang, "异常阻塞", "Blocked") :
    status;

  return (
    <span className={`inline-flex items-center px-1.5 py-0.5 rounded-[2px] text-[10px] font-bold uppercase tracking-widest ${
      isRunning ? "bg-emerald-500/10 text-emerald-500" :
      isPaused ? "bg-amber-500/10 text-amber-500" :
      isDraft ? "bg-blue-500/10 text-blue-500" :
      isBlocked ? "bg-red-500/10 text-red-500" :
      "bg-secondary text-muted-foreground"
    }`}>
      {label}
    </span>
  );
}

async function fetchStrategies(lang: UiLanguage): Promise<{ items: StrategyListResponse["items"]; error: string | null }> {
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
    return { items: [], error: pickText(lang, "策略列表暂时不可用。", "Strategy catalog is temporarily unavailable.") };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
