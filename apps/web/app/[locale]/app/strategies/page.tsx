import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Filter, LayoutGrid, List, Pause, Plus } from "lucide-react";

import { StrategyInventoryTable } from "@/components/strategies/strategy-inventory-table";
import { Button } from "@/components/ui/form";
import { Card } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ notice?: string | string[]; error?: string | string[]; status?: string | string[]; symbol?: string | string[] }>;
};

type StrategyListItem = {
  budget: string;
  id: string;
  market: string;
  name: string;
  status: string;
  symbol: string;
};

type StrategyListResponse = {
  items: StrategyListItem[];
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
    <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-foreground">{t("title")}</h1>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "列表动作已按策略状态裁剪，避免再出现无意义的点击失败。", "Row actions are now filtered by strategy state to avoid meaningless failures.")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <form action="/api/user/strategies/batch" method="post">
            <input name="intent" type="hidden" value="stop-all" />
            <Button className="h-8 border border-red-500/20 bg-red-500/10 px-3 text-xs text-red-500 hover:bg-red-500/20">
              <Pause className="mr-1.5 h-3.5 w-3.5" />
              {pickText(lang, "全部停止", "Stop All")}
            </Button>
          </form>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Plus className="mr-1.5 h-3.5 w-3.5" />
              {pickText(lang, "新建机器人", "New Bot")}
            </Button>
          </Link>
        </div>
      </div>

      <form action={`/${locale}/app/strategies`} className="flex flex-wrap items-center gap-4 rounded-xl border border-border/60 bg-card p-3" method="get">
        <div className="flex max-w-[320px] flex-1 items-center gap-2 rounded-xl border border-border bg-input px-3 py-1.5 transition-colors focus-within:border-primary/50">
          <Filter className="h-4 w-4 text-muted-foreground" />
          <input
            className="w-full border-none bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            defaultValue={symbolFilter}
            name="symbol"
            placeholder={t("filter")}
            type="text"
          />
        </div>
        <select className="h-9 rounded-xl border border-border bg-input px-3 text-xs text-foreground" defaultValue={statusFilter} name="status">
          <option value="all">{pickText(lang, "全部状态", "All Status")}</option>
          <option value="Draft">{pickText(lang, "草稿", "Draft")}</option>
          <option value="Running">{pickText(lang, "运行中", "Running")}</option>
          <option value="Paused">{pickText(lang, "已暂停", "Paused")}</option>
          <option value="ErrorPaused">{pickText(lang, "异常阻塞", "Blocked")}</option>
          <option value="Stopped">{pickText(lang, "已停止", "Stopped")}</option>
        </select>
        <Button className="h-9 px-4 text-xs" type="submit">{pickText(lang, "应用筛选", "Apply Filters")}</Button>
        <div className="ml-auto flex items-center gap-1 rounded-xl border border-border bg-input p-1">
          <button className="rounded-lg bg-secondary p-1.5 text-foreground" type="button">
            <List className="h-4 w-4" />
          </button>
          <button className="rounded-lg p-1.5 text-muted-foreground transition-colors hover:text-foreground" type="button">
            <LayoutGrid className="h-4 w-4" />
          </button>
        </div>
      </form>

      <Card className="border-border bg-card shadow-none">
        <StrategyInventoryTable items={filteredStrategies} lang={lang} locale={locale} />
      </Card>
    </div>
  );
}

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
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
