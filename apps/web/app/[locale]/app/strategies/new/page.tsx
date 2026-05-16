import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Bot, ChevronRight } from "lucide-react";

import { StrategyWorkspaceForm, type StrategyWorkspaceValues } from "@/components/strategies/strategy-workspace-form";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
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
  searchParams?: Promise<{ error?: string | string[]; mode?: string | string[]; symbolQuery?: string | string[] }>;
};

type SymbolSearchResponse = {
  items: Array<{ base_asset: string; market: string; quote_asset: string; symbol: string }>;
};

const FALLBACK_SYMBOLS: SymbolSearchResponse["items"] = [
  { base_asset: "BTC", market: "spot", quote_asset: "USDT", symbol: "BTCUSDT" },
  { base_asset: "ETH", market: "spot", quote_asset: "USDT", symbol: "ETHUSDT" },
  { base_asset: "SOL", market: "spot", quote_asset: "USDT", symbol: "SOLUSDT" },
  { base_asset: "BNB", market: "spot", quote_asset: "USDT", symbol: "BNBUSDT" },
];

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
  const error = firstValue(searchParamsValue.error);
  const modeParam = firstValue(searchParamsValue.mode);
  const displayMode = modeParam === "advanced" ? "advanced" as const : "wizard" as const;
  const symbolQuery = firstValue(searchParamsValue.symbolQuery) ?? "ETHUSDT";
  const results = await Promise.all([fetchTemplates(lang), fetchSymbolMatches(symbolQuery, lang)]);
  const templates = results[0].items;
  const symbolMatches = results[1].items;
  const selectedSymbol = "";
  const selectedMarket = "spot";

  const values: StrategyWorkspaceValues = {
    amountMode: "quote",
    baseQuantity: "0.05",
    batchTakeProfit: "2.0",
    batchTrailing: "",
    coveredRangePercent: "6",
    editorMode: "batch",
    futuresMarginMode: "isolated",
    generation: "arithmetic",
    gridCount: "12",
    gridSpacingPercent: "",
    levelsJson: DEFAULT_LEVELS_JSON,
    leverage: "5",
    lowerRangePercent: "6",
    marketType: selectedMarket,
    mode: selectedMarket === "spot" ? "buy-only" : "long",
    name: "",
    ordinarySide: "lower",
    overallStopLoss: "",
    overallTakeProfit: "4.0",
    postTrigger: "rebuild",
    quoteAmount: "1000",
    referencePrice: "",
    referencePriceMode: "market",
    strategyType: "ordinary_grid",
    symbol: selectedSymbol,
    upperRangePercent: "6",
  };

  return (
    <div className="mx-auto flex h-full max-w-[1800px] flex-col gap-6 pb-12">
      {error ? <StatusBanner description={error} title={pickText(lang, "创建策略失败", "Strategy creation failed")}  tone="info" lang={lang} /> : null}
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-md shadow-primary/20">
          <Bot className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-2xl font-extrabold tracking-tight text-foreground">{t("title")}</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            {pickText(lang, "仅需两步，快速部署您的网格机器人。", "Deploy your grid bot in just two steps.")}
          </p>
        </div>
      </div>

      <StrategyWorkspaceForm
        displayMode={displayMode}
        formAction="/api/user/strategies/create"
        lang={lang}
        searchPath={`/${locale}/app/strategies/new`}
        searchQuery={symbolQuery}
        symbolMatches={symbolMatches}
        values={values}
      />

      <Card className="border-border bg-card">
        <CardHeader className="border-b border-border py-3">
          <CardTitle className="text-sm font-semibold text-foreground">{t("templates.title")}</CardTitle>
        </CardHeader>
        <CardBody className="grid gap-3 p-4 md:grid-cols-2 xl:grid-cols-3">
          {templates.length > 0 ? templates.map((tpl) => (
            <form action="/api/user/strategies/templates" key={tpl.id} method="post" className="w-full">
              <input name="templateId" type="hidden" value={tpl.id} />
              <input name="name" type="hidden" value={`${tpl.name} Copy`} />
              <button className="flex w-full items-center justify-between rounded-2xl border border-border bg-background px-4 py-4 text-left transition-colors hover:border-primary/50 hover:bg-muted/60" type="submit">
                <div>
                  <p className="text-sm font-semibold text-foreground">{tpl.name}</p>
                  <p className="mt-1 text-xs text-muted-foreground">{tpl.symbol} · {describeMarket(lang, tpl.market)}</p>
                </div>
                <ChevronRight className="h-4 w-4 text-muted-foreground" />
              </button>
            </form>
          )) : (
            <div className="rounded-2xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
              {pickText(lang, "当前没有可套用模板。", "No templates are available right now.")}
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  );
}

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function normalizeMarket(market: string): StrategyWorkspaceValues["marketType"] {
  if (market === "coinm") {
    return "coin-m";
  }
  if (market === "usdm") {
    return "usd-m";
  }
  return "spot";
}

function describeMarket(lang: UiLanguage, market: string) {
  switch (market) {
    case "FuturesUsdM":
    case "usdm":
      return pickText(lang, "U 本位合约", "USD-M Futures");
    case "FuturesCoinM":
    case "coinm":
      return pickText(lang, "币本位合约", "COIN-M Futures");
    default:
      return pickText(lang, "现货", "Spot");
  }
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
  if (!query.trim()) return { items: [], error: null };
  if (!sessionToken) return { items: fallbackSymbolMatches(query), error: null };
  const response = await fetch(authApiBaseUrl() + "/exchange/binance/symbols/search", {
    method: "POST",
    headers: { authorization: "Bearer " + sessionToken, "content-type": "application/json" },
    body: JSON.stringify({ query }),
    cache: "no-store",
  });
  if (!response.ok) return { items: fallbackSymbolMatches(query), error: pickText(lang, "搜索失败，已回退到常用交易对。", "Search failed. Showing common symbols instead.") };
  const items = ((await response.json()) as SymbolSearchResponse).items;
  return { items: items.length > 0 ? items : fallbackSymbolMatches(query), error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function fallbackSymbolMatches(query: string) {
  const normalized = query.trim().toLowerCase();
  return FALLBACK_SYMBOLS.filter((item) => {
    return item.symbol.toLowerCase().includes(normalized) || item.base_asset.toLowerCase().includes(normalized) || item.quote_asset.toLowerCase().includes(normalized);
  });
}
