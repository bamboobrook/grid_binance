import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Activity, Bot, ChevronRight, Search } from "lucide-react";

import { Button, Input, Select, Textarea } from "@/components/ui/form";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_LEVELS_JSON = JSON.stringify(
  [
    { entry_price: "1800", quantity: "0.05", take_profit_bps: 180, trailing_bps: null },
    { entry_price: "1827", quantity: "0.05", take_profit_bps: 180, trailing_bps: null },
    { entry_price: "1854", quantity: "0.05", take_profit_bps: 220, trailing_bps: 90 },
  ],
  null,
  2,
);

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ error?: string | string[]; symbolQuery?: string | string[] }>;
};

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
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const t = await getTranslations({ locale, namespace: "newStrategy" });

  const searchParamsValue = (await searchParams) ?? {};
  const symbolQuery = firstValue(searchParamsValue.symbolQuery) ?? "ETHUSDT";
  const results = await Promise.all([fetchTemplates(lang), fetchSymbolMatches(symbolQuery, lang)]);
  const templates = results[0].items;
  const symbolMatches = results[1].items;

  return (
    <div className="flex flex-col h-full space-y-4 max-w-[1480px] mx-auto">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-sm bg-primary/20 flex items-center justify-center text-primary border border-primary/30">
          <Bot className="w-4 h-4" />
        </div>
        <h1 className="text-xl font-bold tracking-tight text-slate-100">{t("title")}</h1>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 items-start">
        <div className="lg:col-span-3 flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none">
            <CardHeader className="py-3 px-4 border-b border-border flex flex-row items-center justify-between">
              <div className="flex items-center gap-2">
                <Activity className="w-4 h-4 text-muted-foreground" />
                <CardTitle className="text-sm text-foreground">{pickText(lang, "图表与策略预览", "Chart & Strategy Visualizer")}</CardTitle>
              </div>
              <div className="text-xs text-muted-foreground font-mono">1D • 4H • 1H • 15M</div>
            </CardHeader>
            <CardBody className="p-0 h-[420px] flex items-center justify-center bg-muted">
              <p className="text-slate-600 text-sm flex flex-col items-center gap-2">
                <LineChartIcon className="w-8 h-8 opacity-50" />
                {pickText(lang, "选择交易对后加载图表预览", "Select a pair to load TradingView chart")}
              </p>
            </CardBody>
          </Card>
        </div>

        <div className="lg:col-span-1 flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none overflow-hidden">
            <div className="bg-secondary/50 px-4 py-2 border-b border-border flex justify-between items-center">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider">{pickText(lang, "参数", "Parameters")}</span>
              <span className="text-[10px] text-primary cursor-default">{pickText(lang, "保存后可继续预检和启动", "Save first, then pre-flight and start")}</span>
            </div>
            <CardBody className="p-4">
              <form action="/api/user/strategies/create" method="post" className="space-y-4">
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "策略名称", "Strategy Name")}</label>
                  <Input defaultValue="ETH Trend Grid" name="name" className="bg-input border-border text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "交易对搜索", "Symbol Search")}</label>
                  <div className="relative group">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground group-focus-within:text-primary transition-colors" />
                    <Input name="symbolQuery" defaultValue={symbolQuery} className="pl-8 bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.symbol")}</label>
                  <Input defaultValue={symbolMatches[0]?.symbol ?? symbolQuery} list="strategy-symbol-suggestions" name="symbol" className="bg-input border-border font-mono text-sm" />
                  <datalist id="strategy-symbol-suggestions">
                    {symbolMatches.map((item) => (
                      <option key={item.symbol} value={item.symbol}>{item.market} · {item.base_asset}/{item.quote_asset}</option>
                    ))}
                  </datalist>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.marketType")}</label>
                    <Select defaultValue="spot" name="marketType" className="bg-input border-border text-xs">
                      <option value="spot">{pickText(lang, "现货", "Spot")}</option>
                      <option value="usd-m">USD-M</option>
                      <option value="coin-m">COIN-M</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.strategyMode")}</label>
                    <Select defaultValue="classic" name="mode" className="bg-input border-border text-xs">
                      <option value="classic">{pickText(lang, "经典", "Classic")}</option>
                      <option value="buy-only">{pickText(lang, "只买", "Buy Only")}</option>
                      <option value="sell-only">{pickText(lang, "只卖", "Sell Only")}</option>
                      <option value="long">{pickText(lang, "做多", "Long")}</option>
                      <option value="short">{pickText(lang, "做空", "Short")}</option>
                      <option value="neutral">{pickText(lang, "中性", "Neutral")}</option>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "生成方式", "Generation")}</label>
                    <Select defaultValue="arithmetic" name="generation" className="bg-input border-border text-xs">
                      <option value="arithmetic">{pickText(lang, "等差", "Arithmetic")}</option>
                      <option value="geometric">{pickText(lang, "等比", "Geometric")}</option>
                      <option value="custom">{pickText(lang, "自定义", "Custom")}</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "编辑模式", "Editor Mode")}</label>
                    <Select defaultValue="batch" name="editorMode" className="bg-input border-border text-xs">
                      <option value="batch">{pickText(lang, "批量生成", "Batch Builder")}</option>
                      <option value="custom">{pickText(lang, "完全自定义", "Custom JSON")}</option>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "计量模式", "Amount Mode")}</label>
                    <Select defaultValue="quote" name="amountMode" className="bg-input border-border text-xs">
                      <option value="quote">{pickText(lang, "按 USDT", "Quote")}</option>
                      <option value="base">{pickText(lang, "按币数量", "Base")}</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "保证金模式", "Margin Mode")}</label>
                    <Select defaultValue="isolated" name="futuresMarginMode" className="bg-input border-border text-xs">
                      <option value="isolated">{pickText(lang, "逐仓", "Isolated")}</option>
                      <option value="cross">{pickText(lang, "全仓", "Cross")}</option>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "杠杆", "Leverage")}</label>
                    <Input defaultValue="5" inputMode="numeric" name="leverage" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.investment")}</label>
                    <Input defaultValue="1000" inputMode="decimal" name="quoteAmount" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "基础币数量", "Base Quantity")}</label>
                    <Input defaultValue="0.05" inputMode="decimal" name="baseQuantity" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "参考价格", "Reference Price")}</label>
                    <Input defaultValue="1800" inputMode="decimal" name="referencePrice" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.gridCount")}</label>
                    <Input defaultValue="20" inputMode="numeric" name="gridCount" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.spacing")}</label>
                    <Input defaultValue="1.5" inputMode="decimal" name="gridSpacingPercent" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.takeProfit")}</label>
                    <Input defaultValue="2.0" inputMode="decimal" name="batchTakeProfit" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "追踪止盈", "Trailing TP")}</label>
                    <Input defaultValue="0.9" inputMode="decimal" name="batchTrailing" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "整体止盈", "Overall TP")}</label>
                    <Input defaultValue="4.0" inputMode="decimal" name="overallTakeProfit" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "整体止损", "Overall SL")}</label>
                    <Input defaultValue="2.0" inputMode="decimal" name="overallStopLoss" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "触发后行为", "Post Trigger")}</label>
                  <Select defaultValue="rebuild" name="postTrigger" className="bg-input border-border text-xs">
                    <option value="stop">{pickText(lang, "停止", "Stop")}</option>
                    <option value="rebuild">{pickText(lang, "重建继续", "Rebuild")}</option>
                  </Select>
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{pickText(lang, "层级 JSON", "Levels JSON")}</label>
                  <Textarea defaultValue={DEFAULT_LEVELS_JSON} name="levels_json" rows={8} className="bg-input border-border font-mono text-xs" />
                </div>

                <div className="pt-2">
                  <Button type="submit" className="w-full font-bold h-10 shadow-lg shadow-primary/20">
                    {t("form.saveDraft") || pickText(lang, "创建机器人", "Create Bot")}
                  </Button>
                </div>
              </form>
            </CardBody>
          </Card>

          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/50 px-4 py-2 border-b border-border flex justify-between items-center">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider">{t("templates.title")}</span>
            </div>
            <CardBody className="p-2 space-y-2 max-h-[220px] overflow-y-auto">
              {templates.length > 0 ? templates.map((tpl) => (
                <form action="/api/user/strategies/templates" key={tpl.id} method="post" className="w-full">
                  <input name="templateId" type="hidden" value={tpl.id} />
                  <input name="name" type="hidden" value={`${tpl.name} Copy`} />
                  <button className="w-full text-left p-2 rounded-sm border border-transparent hover:bg-secondary/80 hover:border-border transition-all flex items-center justify-between group" type="submit">
                    <div>
                      <p className="text-xs font-semibold text-foreground group-hover:text-primary transition-colors">{tpl.name}</p>
                      <p className="text-[10px] text-muted-foreground font-mono">{tpl.symbol} · {tpl.market}</p>
                    </div>
                    <ChevronRight className="w-3 h-3 text-slate-600 group-hover:text-primary" />
                  </button>
                </form>
              )) : (
                <p className="text-[11px] text-muted-foreground text-center py-4">{pickText(lang, "暂无可用模板", "No templates available")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  );
}

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function LineChartIcon(props: Record<string, unknown>) {
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

async function fetchTemplates(lang: UiLanguage): Promise<{ items: TemplateListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return { items: [], error: null };
  const response = await fetch(authApiBaseUrl() + "/strategies/templates", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return { items: [], error: pickText(lang, "加载失败", "Load failed") };
  return { items: ((await response.json()) as TemplateListResponse).items, error: null };
}

async function fetchSymbolMatches(query: string, lang: UiLanguage): Promise<{ items: SymbolSearchResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken || !query.trim()) return { items: [], error: null };
  const response = await fetch(authApiBaseUrl() + "/exchange/binance/symbols/search", {
    method: "POST",
    headers: { authorization: "Bearer " + sessionToken, "content-type": "application/json" },
    body: JSON.stringify({ query }),
    cache: "no-store",
  });
  if (!response.ok) return { items: [], error: pickText(lang, "搜索失败", "Search failed") };
  return { items: ((await response.json()) as SymbolSearchResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
