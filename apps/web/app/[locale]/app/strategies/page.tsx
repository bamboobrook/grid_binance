import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Bot, Plus, Pause, Play, Trash2, Filter, LayoutGrid, List } from "lucide-react";

import { Button, Input } from "@/components/ui/form";
import { Card } from "@/components/ui/card";

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
  const t = await getTranslations({ locale, namespace: 'strategies' });
  const commonT = await getTranslations({ locale, namespace: 'common' });
  
  const searchParamsValue = (await searchParams) ?? {};
  const statusFilter = (Array.isArray(searchParamsValue.status) ? searchParamsValue.status[0] : searchParamsValue.status) ?? "all";
  const symbolFilter = (Array.isArray(searchParamsValue.symbol) ? searchParamsValue.symbol[0] : searchParamsValue.symbol) ?? "";

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
      {/* Header Section */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-slate-100">{t('title')}</h1>
        </div>
        <div className="flex items-center gap-3">
          <form action="/api/user/strategies/batch" method="post">
            <input name="intent" type="hidden" value="stop-all" />
            <Button className="h-8 px-3 text-xs bg-red-500/10 text-red-500 hover:bg-red-500/20 border border-red-500/20">
              <Pause className="w-3.5 h-3.5 mr-1.5" />
              Stop All
            </Button>
          </form>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              New Bot
            </Button>
          </Link>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="bg-[#131b2c] border border-slate-800/60 rounded-sm p-3 flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-slate-900 rounded-sm border border-slate-800 focus-within:border-primary/50 transition-colors flex-1 max-w-[300px]">
          <Filter className="w-4 h-4 text-slate-500" />
          <input 
            type="text" 
            placeholder={t('filter')}
            defaultValue={symbolFilter}
            className="bg-transparent border-none outline-none text-xs w-full text-slate-300 placeholder:text-slate-500"
          />
        </div>
        <div className="flex items-center gap-1 ml-auto bg-slate-900 p-1 rounded-sm border border-slate-800">
          <button className="p-1.5 bg-slate-800 text-slate-200 rounded-sm"><List className="w-4 h-4" /></button>
          <button className="p-1.5 text-slate-500 hover:text-slate-300 rounded-sm transition-colors"><LayoutGrid className="w-4 h-4" /></button>
        </div>
      </div>

      {/* Strategies List */}
      <Card className="bg-[#131b2c] border-slate-800 shadow-none">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="bg-[#0a101d] text-slate-500 text-[10px] uppercase tracking-wider">
              <tr>
                <th className="px-4 py-2 font-medium">{t('table.strategy')}</th>
                <th className="px-4 py-2 font-medium">{t('table.market')}</th>
                <th className="px-4 py-2 font-medium text-center">{t('table.status')}</th>
                <th className="px-4 py-2 font-medium text-right">{t('table.exposure')}</th>
                <th className="px-4 py-2 font-medium text-right">{t('table.actions')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/50">
              {filteredStrategies.length > 0 ? filteredStrategies.map((strategy) => (
                <tr key={strategy.id} className="hover:bg-slate-800/30 transition-colors group">
                  <td className="px-4 py-3">
                    <div className="flex flex-col gap-0.5">
                      <Link href={`/${locale}/app/strategies/${strategy.id}`} className="text-sm font-bold text-slate-200 hover:text-primary transition-colors">
                        {strategy.name}
                      </Link>
                      <span className="text-[10px] text-slate-500 font-mono tracking-wide">{strategy.symbol}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="px-1.5 py-0.5 bg-slate-800 border border-slate-700 text-slate-300 rounded-[2px] text-[10px] font-bold uppercase tracking-widest">
                      {strategy.market}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <StatusBadge status={strategy.status} />
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs text-slate-300 font-semibold">
                    ${strategy.budget}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <Button size="icon" className="h-7 w-7 text-slate-400 hover:text-emerald-500 hover:bg-emerald-500/10">
                        <Play className="w-3.5 h-3.5" />
                      </Button>
                      <Button size="icon" className="h-7 w-7 text-slate-400 hover:text-amber-500 hover:bg-amber-500/10">
                        <Pause className="w-3.5 h-3.5" />
                      </Button>
                      <Button size="icon" className="h-7 w-7 text-slate-400 hover:text-red-500 hover:bg-red-500/10">
                        <Trash2 className="w-3.5 h-3.5" />
                      </Button>
                    </div>
                  </td>
                </tr>
              )) : (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-xs text-slate-500">
                    No active strategies. Create your first bot to get started.
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

function StatusBadge({ status }: { status: string }) {
  const isRunning = status === 'Running';
  const isPaused = status === 'Paused';
  const isDraft = status === 'Draft';
  
  return (
    <span className={`inline-flex items-center px-1.5 py-0.5 rounded-[2px] text-[10px] font-bold uppercase tracking-widest ${
      isRunning ? "bg-emerald-500/10 text-emerald-500" :
      isPaused ? "bg-amber-500/10 text-amber-500" :
      isDraft ? "bg-blue-500/10 text-blue-500" :
      "bg-slate-800 text-slate-400"
    }`}>
      {status}
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
