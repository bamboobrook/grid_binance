import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { AlertTriangle, Bot, ChevronRight, Search } from "lucide-react";

import { Button, Input, Select } from "@/components/ui/form";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

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

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function copy(locale: string, zh: string, en: string) {
  return locale.startsWith("zh") ? zh : en;
}

export default async function StrategyNewPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "newStrategy" });
  const paramsValue = (await searchParams) ?? {};
  const error = firstValue(paramsValue.error);
  const symbolQuery = firstValue(paramsValue.symbolQuery) ?? "BTC";
  const [templatesResult, symbolMatchesResult] = await Promise.all([
    fetchTemplates(),
    fetchSymbolMatches(symbolQuery),
  ]);
  const templates = templatesResult.items;
  const symbolMatches = symbolMatchesResult.items;
  const defaultSymbol = symbolMatches[0]?.symbol ?? "BTCUSDT";

  return (
    <div className="flex flex-col h-full space-y-4 max-w-[1400px] mx-auto">
      {error ? <StatusBanner title={copy(locale, "创建失败", "Create failed")} description={error} /> : null}
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-sm bg-primary/20 flex items-center justify-center text-primary border border-primary/30">
          <Bot className="w-4 h-4" />
        </div>
        <div>
          <h1 className="text-xl font-bold tracking-tight text-slate-100">{t("title")}</h1>
          <p className="text-sm text-muted-foreground">{t("subtitle")}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 items-start">
        <div className="lg:col-span-3 flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none">
            <CardHeader>
              <CardTitle>{copy(locale, "交易对检索", "Symbol search")}</CardTitle>
              <CardDescription>
                {copy(locale, "使用同步后的币安交易对做模糊搜索，现货、U本位、币本位都会一起返回。", "Fuzzy search runs on synced Binance symbols across spot, USD-M, and COIN-M markets.")}
              </CardDescription>
            </CardHeader>
            <CardBody className="space-y-4">
              <form action="" method="get" className="flex flex-col gap-3 md:flex-row md:items-end">
                <div className="flex-1 space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.searchSymbol")}</label>
                  <div className="relative group">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground group-focus-within:text-primary transition-colors" />
                    <Input name="symbolQuery" defaultValue={symbolQuery} className="pl-8 bg-input border-border font-mono text-sm" />
                  </div>
                </div>
                <Button type="submit" className="h-9 px-4">{copy(locale, "搜索", "Search")}</Button>
              </form>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                {symbolMatches.length > 0 ? (
                  symbolMatches.slice(0, 8).map((item) => (
                    <div key={item.market + item.symbol} className="rounded-sm border border-border bg-secondary/30 px-3 py-2 text-sm">
                      <div className="font-semibold text-foreground">{item.symbol}</div>
                      <div className="text-xs text-muted-foreground">{item.market} · {item.base_asset}/{item.quote_asset}</div>
                    </div>
                  ))
                ) : (
                  <div className="rounded-sm border border-dashed border-border px-3 py-6 text-sm text-muted-foreground">
                    {copy(locale, "输入关键字后即可显示匹配交易对。", "Search by keyword to list matching symbols.")}
                  </div>
                )}
              </div>
            </CardBody>
          </Card>
        </div>

        <div className="lg:col-span-1 flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none overflow-hidden">
            <div className="bg-secondary/50 px-4 py-2 border-b border-border flex justify-between items-center">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider">{copy(locale, "策略参数", "Parameters")}</span>
              <span className="text-[10px] text-muted-foreground">{copy(locale, "先保存草稿，再进入详情页精修。", "Save a draft first, then refine it in the detail workspace.")}</span>
            </div>
            <CardBody className="p-4">
              <form action="/api/user/strategies/create" method="post" className="space-y-4">
                <input name="editorMode" type="hidden" value="batch" />
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.strategyName")}</label>
                  <Input name="name" defaultValue={`${defaultSymbol} ${copy(locale, "网格", "Grid")}`} className="bg-input border-border text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.symbol")}</label>
                  <Input name="symbol" defaultValue={defaultSymbol} list="strategy-symbol-suggestions" className="bg-input border-border font-mono text-sm" />
                  <datalist id="strategy-symbol-suggestions">
                    {symbolMatches.map((item) => (
                      <option key={item.market + item.symbol} value={item.symbol}>{item.market} · {item.base_asset}/{item.quote_asset}</option>
                    ))}
                  </datalist>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.marketType")}</label>
                    <Select name="marketType" className="bg-input border-border text-xs">
                      <option value="spot">{copy(locale, "现货", "Spot")}</option>
                      <option value="usd-m">{copy(locale, "U 本位合约", "USD-M Futures")}</option>
                      <option value="coin-m">{copy(locale, "币本位合约", "COIN-M Futures")}</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.strategyMode")}</label>
                    <Select name="mode" className="bg-input border-border text-xs">
                      <option value="classic">{copy(locale, "现货双向", "Spot classic")}</option>
                      <option value="buy-only">{copy(locale, "现货只买", "Spot buy-only")}</option>
                      <option value="sell-only">{copy(locale, "现货只卖", "Spot sell-only")}</option>
                      <option value="long">{copy(locale, "合约做多", "Futures long")}</option>
                      <option value="short">{copy(locale, "合约做空", "Futures short")}</option>
                      <option value="neutral">{copy(locale, "合约中性", "Futures neutral")}</option>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.generationMode")}</label>
                    <Select name="generation" className="bg-input border-border text-xs">
                      <option value="arithmetic">{copy(locale, "等差", "Arithmetic")}</option>
                      <option value="geometric">{copy(locale, "等比", "Geometric")}</option>
                      <option value="custom">{copy(locale, "完全自定义", "Fully custom")}</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "金额模式", "Amount mode")}</label>
                    <Select name="amountMode" className="bg-input border-border text-xs">
                      <option value="quote">{copy(locale, "按 USDT 金额", "By quote amount")}</option>
                      <option value="base">{copy(locale, "按币数量", "By base asset")}</option>
                    </Select>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "保证金模式", "Margin mode")}</label>
                    <Select name="futuresMarginMode" className="bg-input border-border text-xs">
                      <option value="isolated">{copy(locale, "逐仓", "Isolated")}</option>
                      <option value="cross">{copy(locale, "全仓", "Cross")}</option>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.leverage")}</label>
                    <Input name="leverage" type="number" defaultValue="5" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.investment")}</label>
                    <Input name="quoteAmount" type="number" defaultValue="1000" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "单格币数量", "Base quantity")}</label>
                    <Input name="baseQuantity" type="number" defaultValue="0.01" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "参考价格", "Reference price")}</label>
                    <Input name="referencePrice" type="number" step="0.01" defaultValue="100" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.gridCount")}</label>
                    <Input name="gridCount" type="number" defaultValue="10" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.spacing")}</label>
                    <Input name="gridSpacingPercent" type="number" step="0.1" defaultValue="1.5" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{t("form.takeProfit")}</label>
                    <Input name="batchTakeProfit" type="number" step="0.1" defaultValue="2.0" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "追踪止盈 (%)", "Trailing TP (%)")}</label>
                    <Input name="batchTrailing" type="number" step="0.1" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "整体止盈 (%)", "Overall TP (%)")}</label>
                    <Input name="overallTakeProfit" type="number" step="0.1" defaultValue="3.0" className="bg-input border-border font-mono text-sm" />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "整体止损 (%)", "Overall SL (%)")}</label>
                    <Input name="overallStopLoss" type="number" step="0.1" className="bg-input border-border font-mono text-sm" />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "触发后动作", "After trigger")}</label>
                    <Select name="postTrigger" className="bg-input border-border text-xs">
                      <option value="stop">{copy(locale, "停止", "Stop")}</option>
                      <option value="rebuild">{copy(locale, "重建继续", "Rebuild")}</option>
                    </Select>
                  </div>
                </div>

                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">{copy(locale, "自定义网格 JSON", "Custom levels JSON")}</label>
                  <textarea
                    className="ui-input min-h-[140px] font-mono text-xs"
                    name="levels_json"
                    defaultValue={JSON.stringify([
                      { entry_price: "95.00", quantity: "0.0100", take_profit_bps: 150, trailing_bps: null },
                      { entry_price: "100.00", quantity: "0.0100", take_profit_bps: 150, trailing_bps: null },
                      { entry_price: "105.00", quantity: "0.0100", take_profit_bps: 150, trailing_bps: null },
                    ], null, 2)}
                    rows={8}
                  />
                  <div className="flex items-start gap-2 rounded-sm border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
                    <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    <span>{copy(locale, "如果填写追踪止盈，系统会改用 taker 止盈，手续费通常会高于 maker。只有选择“完全自定义”时，以上 JSON 才会参与创建。", "When trailing take profit is set, the runtime must use taker exits and fees may be higher than maker mode. The JSON above is only used when generation is set to Fully custom.")}</span>
                  </div>
                </div>

                <div className="pt-2">
                  <Button type="submit" className="w-full font-bold h-10 shadow-lg shadow-primary/20">
                    {t("form.saveDraft")}
                  </Button>
                </div>
              </form>
            </CardBody>
          </Card>

          <Card className="bg-card border-border shadow-none">
            <CardHeader>
              <CardTitle>{t("templates.title")}</CardTitle>
              <CardDescription>{copy(locale, "管理员模板会复制成你的私有草稿，后续修改不会回写模板本身。", "Admin templates are copied into your own draft and later edits stay private to your strategy.")}</CardDescription>
            </CardHeader>
            <CardBody className="space-y-2">
              {templates.length > 0 ? templates.map((tpl) => (
                <form key={tpl.id} action="/api/user/strategies/templates" method="post" className="flex items-center justify-between rounded-sm border border-border bg-secondary/30 px-3 py-3">
                  <input name="templateId" type="hidden" value={tpl.id} />
                  <input name="name" type="hidden" value={`${tpl.name} Copy`} />
                  <div>
                    <p className="text-sm font-semibold text-foreground">{tpl.name}</p>
                    <p className="text-xs text-muted-foreground">{tpl.market} · {tpl.symbol}</p>
                  </div>
                  <Button type="submit" className="h-8 px-3 text-xs">
                    {t("templates.apply")}
                    <ChevronRight className="ml-1.5 h-3.5 w-3.5" />
                  </Button>
                </form>
              )) : (
                <p className="text-[11px] text-muted-foreground">{copy(locale, "当前没有可用模板。", "No templates are available right now.")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  );
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
