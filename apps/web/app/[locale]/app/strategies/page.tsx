import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Bot, Plus, Pause, Play, Trash2, Filter, LayoutGrid, List } from "lucide-react";

import { Button } from "@/components/ui/form";
import { Chip } from "@/components/ui/chip";

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
    <div className="space-y-6">
      {/* Header Section */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{t('title')}</h1>
          <p className="text-muted-foreground text-sm">
            Manage your active trading bots and monitor their performance.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="bg-amber-500 hover:bg-amber-600 text-white border-none shadow-lg shadow-amber-500/20">
              <Plus className="w-4 h-4 mr-2" />
              {t('new')}
            </Button>
          </Link>
          <form action="/api/user/strategies/batch" method="post">
            <input name="intent" type="hidden" value="stop-all" />
            <Button  className="border-red-500/50 text-red-500 hover:bg-red-500/10">
              <Pause className="w-4 h-4 mr-2" />
              {t('stopAll')}
            </Button>
          </form>
        </div>
      </div>

      {/* Stats Overview */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {[
          { label: commonT('gridConfig.status.running'), count: strategies.filter(s => s.status === 'Running').length, color: 'text-green-500', bg: 'bg-green-500/10' },
          { label: commonT('gridConfig.status.paused'), count: strategies.filter(s => s.status === 'Paused').length, color: 'text-amber-500', bg: 'bg-amber-500/10' },
          { label: commonT('gridConfig.status.draft'), count: strategies.filter(s => s.status === 'Draft').length, color: 'text-blue-500', bg: 'bg-blue-500/10' },
          { label: 'Total Budget', count: `$${strategies.reduce((acc, s) => acc + parseFloat(s.budget || '0'), 0).toFixed(2)}`, color: 'text-foreground', bg: 'bg-muted' }
        ].map((stat, i) => (
          <div key={i} className="bg-card border border-border rounded-xl p-4 flex flex-col gap-1 shadow-sm">
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{stat.label}</span>
            <div className="flex items-center justify-between">
              <span className={`text-xl font-bold ${stat.color}`}>{stat.count}</span>
              <div className={`w-8 h-8 rounded-full ${stat.bg} flex items-center justify-center`}>
                <Bot className={`w-4 h-4 ${stat.color}`} />
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Filter Bar */}
      <div className="bg-card border border-border rounded-xl p-4 flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-muted/50 rounded-lg border border-transparent focus-within:border-amber-500/50 transition-colors flex-1 min-w-[200px]">
          <Filter className="w-4 h-4 text-muted-foreground" />
          <input 
            type="text" 
            placeholder={t('filter')}
            defaultValue={symbolFilter}
            className="bg-transparent border-none outline-none text-sm w-full"
          />
        </div>
        <div className="flex items-center gap-2">
          <Button   className="bg-muted text-foreground"><List className="w-4 h-4" /></Button>
          <Button  ><LayoutGrid className="w-4 h-4" /></Button>
        </div>
      </div>

      {/* Strategies List */}
      <div className="bg-card border border-border rounded-xl overflow-hidden shadow-sm">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground font-medium border-b border-border">
            <tr>
              <th className="px-6 py-3">{t('table.strategy')}</th>
              <th className="px-6 py-3">{t('table.market')}</th>
              <th className="px-6 py-3">{t('table.status')}</th>
              <th className="px-6 py-3 text-right">{t('table.exposure')}</th>
              <th className="px-6 py-3 text-right">{t('table.actions')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {filteredStrategies.length > 0 ? filteredStrategies.map((strategy) => (
              <tr key={strategy.id} className="hover:bg-muted/30 transition-colors group">
                <td className="px-6 py-4">
                  <div className="flex flex-col">
                    <Link href={`/${locale}/app/strategies/${strategy.id}`} className="font-semibold text-foreground hover:text-amber-500 transition-colors">
                      {strategy.name}
                    </Link>
                    <span className="text-xs text-muted-foreground">{strategy.symbol}</span>
                  </div>
                </td>
                <td className="px-6 py-4">
                  <span className="px-2 py-0.5 bg-muted rounded text-[10px] font-bold uppercase tracking-tight">
                    {strategy.market}
                  </span>
                </td>
                <td className="px-6 py-4">
                  <StatusBadge status={strategy.status} t={commonT} />
                </td>
                <td className="px-6 py-4 text-right font-mono font-medium">
                  {strategy.budget}
                </td>
                <td className="px-6 py-4 text-right">
                  <div className="flex items-center justify-end gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                    <Button   className="h-8 w-8 p-0 hover:text-green-500">
                      <Play className="w-4 h-4" />
                    </Button>
                    <Button   className="h-8 w-8 p-0 hover:text-amber-500">
                      <Pause className="w-4 h-4" />
                    </Button>
                    <Button   className="h-8 w-8 p-0 hover:text-red-500">
                      <Trash2 className="w-4 h-4" />
                    </Button>
                  </div>
                </td>
              </tr>
            )) : (
              <tr>
                <td colSpan={5} className="px-6 py-12 text-center text-muted-foreground">
                  No strategies found. Create your first bot to get started!
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function StatusBadge({ status, t }: { status: string, t: any }) {
  const styles: Record<string, string> = {
    Running: "bg-green-500/10 text-green-500 border-green-500/20",
    Paused: "bg-amber-500/10 text-amber-500 border-amber-500/20",
    Draft: "bg-blue-500/10 text-blue-500 border-blue-500/20",
    Stopped: "bg-red-500/10 text-red-500 border-red-500/20",
  };

  const label: Record<string, string> = {
    Running: t('gridConfig.status.running'),
    Paused: t('gridConfig.status.paused'),
    Draft: t('gridConfig.status.draft'),
    Stopped: t('gridConfig.status.stopped'),
  };

  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-bold border ${styles[status] || styles.Draft}`}>
      {label[status] || status}
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
