import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Bot, Search, Save, Copy, Info, AlertTriangle, ChevronRight } from "lucide-react";

import { Button } from "../../../../../components/ui/form";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ error?: string | string[]; symbolQuery?: string | string[] }>;
};

type StrategyListResponse = { items: Array<{ id: string }> };

type SymbolSearchResponse = {
  items: Array<{ base_asset: string; market: string; quote_asset: string; symbol: string }>;
};

type TemplateListResponse = {
  items: Array<{
    id: string;
    market: string;
    name: string;
    symbol: string;
  }>;
};

export default async function StrategyNewPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'newStrategy' });
  const commonT = await getTranslations({ locale, namespace: 'common' });
  
  const searchParamsValue = (await searchParams) ?? {};
  const symbolQuery = (Array.isArray(searchParamsValue.symbolQuery) ? searchParamsValue.symbolQuery[0] : searchParamsValue.symbolQuery) ?? "";
  
  const results = await Promise.all([fetchStrategies(), fetchTemplates(), fetchSymbolMatches(symbolQuery)]);
  const strategies = results[0].items;
  const templates = results[1].items;
  const symbolMatches = results[2].items;

  return (
    <div className="max-w-6xl mx-auto space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="w-10 h-10 rounded-xl bg-amber-500/20 flex items-center justify-center text-amber-500">
          <Bot className="w-6 h-6" />
        </div>
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{t('title')}</h1>
          <p className="text-muted-foreground text-sm">{t('subtitle')}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Config Panel */}
        <div className="lg:col-span-2 space-y-6">
          <div className="bg-card border border-border rounded-2xl p-6 shadow-sm space-y-6">
            <h2 className="text-lg font-bold flex items-center gap-2">
              <ChevronRight className="w-4 h-4 text-amber-500" />
              General Settings
            </h2>
            
            <form action="/api/user/strategies/create" method="post" className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.strategyName')}</label>
                  <input name="name" defaultValue="My First Grid Bot" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.symbol')}</label>
                  <div className="relative group">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground group-focus-within:text-amber-500 transition-colors" />
                    <input name="symbol" defaultValue="ETHUSDT" className="w-full bg-muted/50 border border-border rounded-lg pl-10 pr-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                  </div>
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.marketType')}</label>
                  <select name="marketType" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors appearance-none">
                    <option value="spot">Binance Spot</option>
                    <option value="usd-m">USDT-Futures</option>
                  </select>
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.strategyMode')}</label>
                  <select name="mode" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors appearance-none">
                    <option value="classic">Classic</option>
                    <option value="long">Long (Futures)</option>
                    <option value="short">Short (Futures)</option>
                  </select>
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.leverage')}</label>
                  <input name="leverage" type="number" defaultValue="1" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                </div>
              </div>

              <div className="h-[1px] bg-border my-2" />
              
              <h2 className="text-lg font-bold flex items-center gap-2 pt-2">
                <ChevronRight className="w-4 h-4 text-amber-500" />
                Grid Parameters
              </h2>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.investment')}</label>
                  <input name="quoteAmount" type="number" defaultValue="1000" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm font-mono focus:border-amber-500/50 outline-none transition-colors" />
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.gridCount')}</label>
                  <input name="gridCount" type="number" defaultValue="10" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.spacing')}</label>
                  <input name="gridSpacingPercent" type="number" step="0.1" defaultValue="1.5" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                </div>
                <div className="space-y-1.5">
                  <label className="text-xs font-bold text-muted-foreground uppercase">{t('form.takeProfit')}</label>
                  <input name="batchTakeProfit" type="number" step="0.1" defaultValue="2.0" className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2 text-sm focus:border-amber-500/50 outline-none transition-colors" />
                </div>
              </div>

              <div className="pt-4">
                <Button type="submit" className="w-full bg-amber-500 hover:bg-amber-600 text-white font-bold py-6 text-base rounded-xl border-none shadow-xl shadow-amber-500/20 transition-all">
                  <Save className="w-5 h-5 mr-2" />
                  {t('form.saveDraft')}
                </Button>
              </div>
            </form>
          </div>
        </div>

        {/* Templates & Help Side Panel */}
        <div className="space-y-6">
          <div className="bg-card border border-border rounded-2xl p-6 shadow-sm space-y-6">
            <h2 className="font-bold flex items-center gap-2">
              <Copy className="w-4 h-4 text-blue-500" />
              {t('templates.title')}
            </h2>
            <div className="space-y-3">
              {templates.length > 0 ? templates.map(tpl => (
                <button key={tpl.id} className="w-full text-left p-3 rounded-xl border border-border bg-muted/20 hover:border-blue-500/50 transition-all group">
                  <p className="text-xs font-bold group-hover:text-blue-500">{tpl.name}</p>
                  <p className="text-[10px] text-muted-foreground">{tpl.symbol} · {tpl.market}</p>
                </button>
              )) : (
                <p className="text-xs text-muted-foreground italic text-center py-4 bg-muted/20 rounded-xl">No templates found</p>
              )}
            </div>
            <Button  className="w-full text-xs">Create Template</Button>
          </div>

          <div className="bg-amber-500/10 border border-amber-500/20 rounded-2xl p-6 space-y-3">
            <div className="flex items-center gap-2 text-amber-500 font-bold text-sm">
              <AlertTriangle className="w-4 h-4" />
              Risk Notice
            </div>
            <ul className="text-[11px] text-muted-foreground space-y-2 list-disc pl-4 leading-relaxed">
              <li>Grid trading requires sufficient balance in your wallet.</li>
              <li>Trailing Take Profit may involve higher slippage.</li>
              <li>Always perform a Pre-flight check before starting.</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}

async function fetchStrategies(): Promise<{ items: StrategyListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return { items: [], error: null };
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return { items: [], error: "Load failed" };
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

async function fetchTemplates(): Promise<{ items: TemplateListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return { items: [], error: null };
  const response = await fetch(authApiBaseUrl() + "/strategies/templates", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return { items: [], error: "Load failed" };
  return { items: ((await response.json()) as TemplateListResponse).items, error: null };
}

async function fetchSymbolMatches(query: string): Promise<{ items: SymbolSearchResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken || !query.trim()) return { items: [], error: null };
  const response = await fetch(authApiBaseUrl() + "/exchange/binance/symbols/search", {
    method: "POST",
    headers: { authorization: "Bearer " + sessionToken, "content-type": "application/json" },
    body: JSON.stringify({ query }),
    cache: "no-store",
  });
  if (!response.ok) return { items: [], error: "Search failed" };
  return { items: ((await response.json()) as SymbolSearchResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
