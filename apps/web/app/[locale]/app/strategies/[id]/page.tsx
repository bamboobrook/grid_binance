import { cookies } from "next/headers";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { LivePriceDisplay } from "@/components/ui/live-price-display";
import { StrategyStatusBadge } from "@/components/ui/strategy-status-badge";
import { StrategyWorkspaceForm, type StrategyWorkspaceValues } from "@/components/strategies/strategy-workspace-form";
import { formatPnl, formatPrice, pnlColor } from "@/lib/ui/format";
import { formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type RuntimeEvent = {
  event_type: string;
  timestamp: string;
  detail?: string;
};

type RuntimeFill = {
  fill_id: string;
  price: string;
  quantity: string;
  side: string;
  realized_pnl: string | null;
  timestamp: string;
};

type RuntimePosition = {
  market: string;
  mode: string;
  quantity: string;
  average_entry_price: string;
};

type StrategyDetail = {
  id: string;
  name: string;
  status: string;
  symbol: string;
  market: string;
  strategy_type: string;
  budget: string;
  reference_price: string | null;
  reference_price_source: string | null;
  draft_revision: { reference_price_source: string | null };
  grid_count: number;
  lower_price: string;
  upper_price: string;
  realized_pnl: string | null;
  unrealized_pnl: string | null;
  net_pnl: string | null;
  tags: string[];
  notes: string;
  runtime?: {
    events?: RuntimeEvent[];
    fills?: RuntimeFill[];
    positions?: RuntimePosition[];
  };
};

export default async function StrategyDetailPage({
  params,
}: {
  params: Promise<{ locale: string; id: string }>;
}) {
  const { locale, id } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  const strategy = sessionToken ? await fetchStrategy(sessionToken, id) : null;
  if (!strategy) {
    return (
      <div className="space-y-6">
        <h1 className="text-2xl font-bold">{pickText(lang, "策略详情", "Strategy Detail")}</h1>
        <p className="text-muted-foreground">{pickText(lang, "策略不存在或未登录。", "Strategy not found or not logged in.")}</p>
      </div>
    );
  }

  const realizedPnl = strategy.realized_pnl ? Number(strategy.realized_pnl) : null;
  const netPnl = strategy.net_pnl ? Number(strategy.net_pnl) : null;
  const refPrice = strategy.reference_price ? Number(strategy.reference_price) : null;
  const strategyType = mapStrategyTypeToForm(strategy.strategy_type);
  const referencePriceMode = mapReferencePriceModeToForm(strategy.draft_revision.reference_price_source);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold" data-testid="strategy-detail-header">
            {strategy.name}
          </h1>
          <StrategyStatusBadge status={strategy.status} lang={lang} />
        </div>
        <div className="flex gap-2">
          {strategy.status === "Running" && (
            <>
              <form action={`/api/user/strategies/${id}`} method="post">
                <input name="intent" type="hidden" value="pause" />
                <button className="inline-flex items-center rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-1.5 text-sm font-medium text-amber-500 hover:bg-amber-500/20" type="submit">
                  {pickText(lang, "暂停策略", "Pause Strategy")}
                </button>
              </form>
              <form action={`/api/user/strategies/${id}`} method="post">
                <input name="intent" type="hidden" value="stop" />
                <button className="inline-flex items-center rounded-md border border-red-500/20 bg-red-500/10 px-3 py-1.5 text-sm font-medium text-red-500 hover:bg-red-500/20" type="submit">
                  {pickText(lang, "停止策略", "Stop Strategy")}
                </button>
              </form>
            </>
          )}
          {(strategy.status === "Paused" || strategy.status === "ErrorPaused") && (
            <>
              <form action={`/api/user/strategies/${id}`} method="post">
                <input name="intent" type="hidden" value="start" />
                <button className="inline-flex items-center rounded-md border border-emerald-500/20 bg-emerald-500/10 px-3 py-1.5 text-sm font-medium text-emerald-500 hover:bg-emerald-500/20" type="submit">
                  {pickText(lang, "恢复", "Resume")}
                </button>
              </form>
              <form action={`/api/user/strategies/${id}`} method="post">
                <input name="intent" type="hidden" value="stop" />
                <button className="inline-flex items-center rounded-md border border-red-500/20 bg-red-500/10 px-3 py-1.5 text-sm font-medium text-red-500 hover:bg-red-500/20" type="submit">
                  {pickText(lang, "停止策略", "Stop Strategy")}
                </button>
              </form>
            </>
          )}
          <a
            href={`/${locale}/app/strategies/new?mode=advanced&clone=${id}`}
            className="inline-flex items-center rounded-md border px-3 py-1.5 text-sm font-medium hover:bg-muted"
          >
            {pickText(lang, "复制策略", "Clone Strategy")}
          </a>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-3">
        <div className="rounded-lg border p-4">
          <p className="text-xs text-muted-foreground">
            {pickText(lang, "当前价格", "Current Price")}
          </p>
          <LivePriceDisplay symbol={strategy.symbol ?? id} lang={lang} />
        </div>
        <div className="rounded-lg border p-4">
          <p className="text-xs text-muted-foreground">
            {pickText(lang, "参考价", "Reference Price")}
          </p>
          <p className="mt-1 text-lg font-semibold">{formatPrice(refPrice)}</p>
        </div>
        <div className="rounded-lg border p-4">
          <p className="text-xs text-muted-foreground">
            {pickText(lang, "累计收益", "Total PnL")}
          </p>
          <p className={`mt-1 text-lg font-semibold ${pnlColor(netPnl)}`}>
            {formatPnl(netPnl)}
          </p>
        </div>
      </div>

      <div className="rounded-lg border p-4">
        <h2 className="mb-3 text-sm font-medium text-muted-foreground">
          {pickText(lang, "策略信息", "Strategy Info")}
        </h2>
        <dl className="grid gap-2 text-sm sm:grid-cols-2 lg:grid-cols-3" data-testid="strategy-detail-info">
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "策略ID", "Strategy ID")}</dt>
            <dd className="font-mono">{id}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "交易对", "Symbol")}</dt>
            <dd className="font-mono font-medium">{strategy.symbol}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "市场", "Market")}</dt>
            <dd>{describeMarket(lang, strategy.market)}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "策略类型", "Strategy Type")}</dt>
            <dd>{describeStrategyType(lang, strategy.strategy_type)}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "预算", "Budget")}</dt>
            <dd>{strategy.budget}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "网格数量", "Grid Count")}</dt>
            <dd>{strategy.grid_count}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">{pickText(lang, "价格区间", "Price Range")}</dt>
            <dd>{strategy.lower_price} — {strategy.upper_price}</dd>
          </div>
          {strategy.tags.length > 0 && (
            <div>
              <dt className="text-muted-foreground">{pickText(lang, "标签", "Tags")}</dt>
              <dd className="flex flex-wrap gap-1">
                {strategy.tags.map((tag) => (
                  <span key={tag} className="rounded-full border bg-secondary px-2 py-0.5 text-xs">{tag}</span>
                ))}
              </dd>
            </div>
          )}
          {strategy.notes && (
            <div className="sm:col-span-2">
              <dt className="text-muted-foreground">{pickText(lang, "备注", "Notes")}</dt>
              <dd>{strategy.notes}</dd>
            </div>
          )}
        </dl>
      </div>

      <div className="rounded-lg border p-4">
        <h2 className="mb-3 text-sm font-medium text-muted-foreground">
          {pickText(lang, "运行事件", "Runtime Events")}
        </h2>
        {strategy.runtime?.events && strategy.runtime.events.length > 0 ? (
          <ul className="space-y-2 text-sm mb-4">
            {strategy.runtime.events.slice(0, 10).map((event, idx) => (
              <li key={idx} className="flex items-center justify-between">
                <span>{describeRuntimeEventDetail(lang, event.event_type)}</span>
                <span className="text-xs text-muted-foreground">{formatTaipeiDateTime(event.timestamp, lang)}</span>
              </li>
            ))}
          </ul>
        ) : strategy.status !== "Draft" ? (
          <p className="text-xs text-muted-foreground mb-2">
            {describeRuntimeEventDetail(lang, strategy.status === "Running" ? "GridFill" : "StrategyPaused")}
          </p>
        ) : null}

        {strategy.runtime?.positions && strategy.runtime.positions.length > 0 && (
          <div className="mb-4">
            <h3 className="text-xs font-medium text-muted-foreground mb-2">{pickText(lang, "当前持仓", "Open Positions")}</h3>
            <ul className="space-y-1 text-sm">
              {strategy.runtime.positions.map((pos, idx) => (
                <li key={idx} className="flex items-center justify-between">
                  <span>{pos.quantity} @ {formatPrice(Number(pos.average_entry_price))}</span>
                  <span className="text-xs text-muted-foreground">{describeMarket(lang, pos.market)}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {strategy.runtime?.fills && strategy.runtime.fills.length > 0 && (
          <div className="mb-4">
            <h3 className="text-xs font-medium text-muted-foreground mb-2">{pickText(lang, "最近成交", "Recent Fills")}</h3>
            <ul className="space-y-1 text-sm">
              {strategy.runtime.fills.slice(0, 10).map((fill) => (
                <li key={fill.fill_id} className="flex items-center justify-between">
                  <span className={fill.side === "Buy" ? "text-emerald-500" : "text-red-500"}>
                    {fill.side} {fill.quantity} @ {formatPrice(Number(fill.price))}
                  </span>
                  <span className="text-xs text-muted-foreground">{formatTaipeiDateTime(fill.timestamp, lang)}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
        <StrategyWorkspaceForm
          displayMode="advanced"
          editingLocked={strategy.status === "Running"}
          formAction={`/api/user/strategies/${id}`}
          lang={lang}
          searchPath={`/${locale}/app/strategies/new`}
          searchQuery={strategy.symbol}
          symbolMatches={[]}
          values={{
            amountMode: "quote",
            baseQuantity: "0.05",
            batchTakeProfit: "2.0",
            batchTrailing: "",
            coveredRangePercent: "6",
            editorMode: "batch",
            futuresMarginMode: "isolated",
            generation: "arithmetic",
            gridCount: String(strategy.grid_count),
            gridSpacingPercent: "",
            levelsJson: "[]",
            leverage: "5",
            lowerRangePercent: "6",
            marketType: strategy.market === "FuturesUsdM" ? "usd-m" : strategy.market === "FuturesCoinM" ? "coin-m" : "spot",
            mode: strategy.market === "Spot" ? "buy-only" : "long",
            name: strategy.name,
            ordinarySide: "lower",
            overallStopLoss: "",
            overallTakeProfit: "4.0",
            postTrigger: "rebuild",
            quoteAmount: strategy.budget,
            referencePrice: strategy.reference_price ?? "",
            referencePriceMode: mapReferencePriceModeToForm(strategy.draft_revision.reference_price_source) as "manual" | "market",
            strategyType: strategyType as "ordinary_grid" | "classic_bilateral_grid",
            symbol: strategy.symbol,
            upperRangePercent: "6",
          }}
        />
      </div>
    </div>
  );
}

function describeMarket(lang: UiLanguage, market: string) {
  switch (market) {
    case "Spot": return pickText(lang, "现货", "Spot");
    case "FuturesUsdM": return pickText(lang, "U本位合约", "USD-M Futures");
    case "FuturesCoinM": return pickText(lang, "币本位合约", "COIN-M Futures");
    default: return market;
  }
}

function describeStrategyType(lang: UiLanguage, strategyType: string) {
  switch (strategyType) {
    case "SpotGrid": return pickText(lang, "普通网格", "Spot Grid");
    case "FuturesLong": return pickText(lang, "合约做多网格", "Futures Long Grid");
    case "FuturesShort": return pickText(lang, "合约做空网格", "Futures Short Grid");
    case "ClassicBilateralSpot": return pickText(lang, "经典双向网格", "Classic Bilateral Grid");
    case "ClassicBilateralFutures": return pickText(lang, "经典双向合约网格", "Classic Bilateral Futures");
    default: return strategyType;
  }
}

function mapStrategyTypeToForm(backendType: string): string {
  switch (backendType) {
    case "SpotGrid": return "ordinary_grid";
    case "FuturesLong": return "ordinary_grid";
    case "FuturesShort": return "ordinary_grid";
    case "ClassicBilateralSpot": return "classic_bilateral_grid";
    case "ClassicBilateralFutures": return "classic_bilateral_grid";
    default: return "ordinary_grid";
  }
}

function mapReferencePriceModeToForm(source: string | null): string {
  switch (source) {
    case "last_price": return "market";
    case "manual": return "manual";
    default: return "market";
  }
}

function describeRuntimeEventDetail(lang: UiLanguage, eventType: string): string {
  switch (eventType) {
    case "GridFill": return pickText(lang, "网格成交", "Grid Fill");
    case "TakeProfit": return pickText(lang, "止盈触发", "Take Profit Triggered");
    case "StopLoss": return pickText(lang, "止损触发", "Stop Loss Triggered");
    case "PositionClosed": return pickText(lang, "仓位平仓", "Position Closed");
    case "StrategyPaused": return pickText(lang, "策略暂停", "Strategy Paused");
    case "StrategyStopped": return pickText(lang, "策略停止", "Strategy Stopped");
    default: return eventType;
  }
}

async function fetchStrategy(sessionToken: string, strategyId: string): Promise<StrategyDetail | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/strategies/" + strategyId, {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as StrategyDetail;
  } catch {
    return null;
  }
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
