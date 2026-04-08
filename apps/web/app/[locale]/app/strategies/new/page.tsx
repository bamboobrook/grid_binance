import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Bot, Search, Save, Copy, AlertTriangle, ChevronRight, Activity } from "lucide-react";
import { Button, Input, Select } from "@/components/ui/form";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";

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
  
  const searchParamsValue = (await searchParams) ?? {};
  const symbolQuery = (Array.isArray(searchParamsValue.symbolQuery) ? searchParamsValue.symbolQuery[0] : searchParamsValue.symbolQuery) ?? "";
  
  const results = await Promise.all([fetchStrategies(), fetchTemplates(), fetchSymbolMatches(symbolQuery)]);
  const templates = results[1].items;

  return (
    <div className="flex flex-col h-full space-y-4 max-w-[1400px] mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-sm bg-primary/20 flex items-center justify-center text-primary border border-primary/30">
            <Bot className="w-4 h-4" />
          </div>
          <h1 className="text-xl font-bold tracking-tight text-slate-100">{t('title')}</h1>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 items-start">
        {/* Left Panel: Chart Placeholder (Takes up 2/3 of space on large screens) */}
        <div className="lg:col-span-3 flex flex-col gap-4">
          {/* Main Chart Card */}
          <Card className="bg-card border-border shadow-none">
            <CardHeader className="py-3 px-4 border-b border-border flex flex-row items-center justify-between">
              <div className="flex items-center gap-2">
                <Activity className="w-4 h-4 text-muted-foreground" />
                <CardTitle className="text-sm text-foreground">Chart & Strategy Visualizer</CardTitle>
              </div>
              <div className="text-xs text-muted-foreground font-mono">1D • 4H • 1H • 15M</div>
            </CardHeader>
            <CardBody className="p-0 h-[400px] flex items-center justify-center bg-muted">
              <p className="text-slate-600 text-sm flex flex-col items-center gap-2">
                <LineChartIcon className="w-8 h-8 opacity-50" />
                Select a pair to load TradingView chart
              </p>
            </CardBody>
          </Card>

          {/* Additional info or Backtest summary could go here */}
        </div>

        {/* Right Panel: High Density Configuration Form (Takes 1/3) */}
        <div className="lg:col-span-1 flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none overflow-hidden">
            <div className="bg-secondary/50 px-4 py-2 border-b border-border flex justify-between items-center">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider">Parameters</span>
              <span className="text-[10px] text-primary cursor-pointer hover:underline">Reset</span>
            </div>
            <CardBody className="p-4">
              <form action="/api/user/strategies/create" method="post" className="space-y-4">
                
                {/* Pair Selection */}
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.symbol')}</label>
                  <div className="relative group">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground group-focus-within:text-primary transition-colors" />
                    <Input name="symbol" defaultValue="ETHUSDT" className="pl-8 bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.marketType')}</label>
                    <Select name="marketType" className="bg-input border-border text-xs">
                      <option value="spot">Spot</option>
                      <option value="usd-m">Futures</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.strategyMode')}</label>
                    <Select name="mode" className="bg-input border-border text-xs">
                      <option value="classic">Classic</option>
                      <option value="long">Long</option>
                      <option value="short">Short</option>
                    </Select>
                  </div>
                </div>

                <div className="h-px bg-secondary my-4" />

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.investment')}</label>
                  <div className="relative">
                    <Input name="quoteAmount" type="number" defaultValue="1000" className="pr-12 bg-input border-border font-mono" />
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">USDT</span>
                  </div>
                  {/* Slider representation */}
                  <div className="h-1 w-full bg-secondary rounded-full mt-2 relative">
                    <div className="absolute left-0 top-0 h-full w-1/3 bg-primary rounded-full"></div>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.gridCount')}</label>
                    <Input name="gridCount" type="number" defaultValue="20" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.spacing')}</label>
                    <div className="relative">
                      <Input name="gridSpacingPercent" type="number" step="0.1" defaultValue="1.5" className="bg-input border-border font-mono text-sm pr-6" />
                      <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">%</span>
                    </div>
                  </div>
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t('form.takeProfit')}</label>
                  <div className="relative">
                    <Input name="batchTakeProfit" type="number" step="0.1" defaultValue="2.0" className="bg-input border-border font-mono text-sm pr-6" />
                    <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">%</span>
                  </div>
                </div>

                <div className="pt-2">
                  <Button type="submit" className="w-full font-bold h-10 shadow-lg shadow-primary/20">
                    {t('form.saveDraft') || 'Create Bot'}
                  </Button>
                </div>
              </form>
            </CardBody>
          </Card>

          {/* Templates Section */}
          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/50 px-4 py-2 border-b border-border flex justify-between items-center">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider">{t('templates.title')}</span>
            </div>
            <CardBody className="p-2 space-y-1 max-h-[150px] overflow-y-auto">
              {templates.length > 0 ? templates.map(tpl => (
                <button key={tpl.id} className="w-full text-left p-2 rounded-sm border border-transparent hover:bg-secondary/80 hover:border-border transition-all flex items-center justify-between group">
                  <div>
                    <p className="text-xs font-semibold text-foreground group-hover:text-primary transition-colors">{tpl.name}</p>
                    <p className="text-[10px] text-muted-foreground font-mono">{tpl.symbol}</p>
                  </div>
                  <ChevronRight className="w-3 h-3 text-slate-600 group-hover:text-primary" />
                </button>
              )) : (
                <p className="text-[11px] text-muted-foreground text-center py-4">No templates available</p>
              )}
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  );
}

function LineChartIcon(props: any) {
  return (
    <svg
      {...props}
      xmlns="http://www.w3.org/2000/svg"
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M3 3v18h18" />
      <path d="m19 9-5 5-4-4-3 3" />
    </svg>
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
