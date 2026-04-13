import Link from "next/link";
import { cookies } from "next/headers";
import { notFound } from "next/navigation";

import { StrategyWorkspaceForm, type StrategyWorkspaceIntentButton, type StrategyWorkspaceValues } from "@/components/strategies/strategy-workspace-form";
import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string; id: string }>;
  searchParams?: Promise<{
    error?: string | string[];
    notice?: string | string[];
    reason?: string | string[];
    step?: string | string[];
    symbolQuery?: string | string[];
  }>;
};

type BackendStrategy = {
  budget: string;
  draft_revision: {
    amount_mode?: "Quote" | "Base";
    futures_margin_mode?: "Isolated" | "Cross" | null;
    generation: string;
    leverage?: number | null;
    levels: Array<{
      entry_price: string;
      quantity: string;
      take_profit_bps: number;
      trailing_bps: number | null;
    }>;
    overall_stop_loss_bps: number | null;
    overall_take_profit_bps: number | null;
    post_trigger_action: string;
  };
  id: string;
  market: string;
  mode: string;
  name: string;
  runtime: {
    events: Array<{ created_at: string; detail: string; event_type: string }>;
    fills: Array<{ realized_pnl: string | null }>;
    orders: Array<unknown>;
    positions: Array<{ average_entry_price: string; quantity: string }>;
  };
  status: string;
  symbol: string;
};

type PreflightReport = {
  failures: Array<{ reason?: string; step: string }>;
  ok: boolean;
  steps: Array<{ reason?: string | null; status: string; step: string }>;
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

type AnalyticsReport = {
  strategies: Array<{
    cost_basis: string;
    current_state: string;
    fees_paid: string;
    fill_count: number;
    funding_total: string;
    net_pnl: string;
    order_count: number;
    position_quantity: string;
    average_entry_price: string;
    long_position_quantity?: string;
    long_average_entry_price?: string;
    short_position_quantity?: string;
    short_average_entry_price?: string;
    realized_pnl: string;
    strategy_id: string;
    unrealized_pnl: string;
  }>;
};

export default async function StrategyDetailPage({ params, searchParams }: PageProps) {
  const { locale, id } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const [strategyResult, analyticsResult] = await Promise.all([fetchStrategy(id, lang), fetchAnalytics(lang)]);
  const strategy = strategyResult.strategy;
  const analytics = analyticsResult.analytics;
  const paramsValue = (await searchParams) ?? {};
  const notice = firstValue(paramsValue.notice);
  const symbolQuery = firstValue(paramsValue.symbolQuery) ?? strategy?.symbol ?? "";
  const error = firstValue(paramsValue.error);
  const reason = firstValue(paramsValue.reason);
  const step = firstValue(paramsValue.step);
  const [preflightResult, symbolMatchesResult] = await Promise.all([
    fetchPreflight(id, lang),
    fetchSymbolMatches(firstValue(paramsValue.symbolQuery) ?? strategy?.symbol ?? "", lang),
  ]);
  const preflight = preflightResult.preflight;

  if (!strategy && !strategyResult.error) {
    notFound();
  }

  if (!strategy) {
    return <StatusBanner title={localize(lang, "策略工作台不可用", "Strategy workspace unavailable")} description={strategyResult.error ?? localize(lang, "策略工作台暂不可用。", "Strategy workspace is temporarily unavailable.")} />;
  }

  const stats = analytics?.strategies.find((item) => item.strategy_id === strategy.id) ?? null;
  const firstLevel = strategy.draft_revision.levels[0];
  const levelsJson = JSON.stringify(strategy.draft_revision.levels, null, 2);
  const detailPagePath = withLocale(locale, `/app/strategies/${strategy.id}`);
  const values: StrategyWorkspaceValues = {
    amountMode: strategy.draft_revision.amount_mode === "Base" ? "base" : "quote",
    baseQuantity: firstLevel?.quantity ?? "",
    batchTakeProfit: firstLevel ? formatBps(firstLevel.take_profit_bps) : "",
    batchTrailing: firstLevel?.trailing_bps ? formatBps(firstLevel.trailing_bps) : "",
    editorMode: strategy.draft_revision.generation === "Custom" ? "custom" : "batch",
    futuresMarginMode: strategy.draft_revision.futures_margin_mode === "Cross" ? "cross" : "isolated",
    generation: mapGenerationToForm(strategy.draft_revision.generation),
    gridCount: String(strategy.draft_revision.levels.length || 0),
    gridSpacingPercent: computeSpacingPercent(strategy.draft_revision.levels[0]?.entry_price, strategy.draft_revision.levels[1]?.entry_price),
    levelsJson,
    leverage: strategy.draft_revision.leverage ? String(strategy.draft_revision.leverage) : "5",
    marketType: mapMarketToForm(strategy.market),
    mode: mapModeToForm(strategy.mode),
    name: strategy.name,
    overallStopLoss: formatBps(strategy.draft_revision.overall_stop_loss_bps),
    overallTakeProfit: formatBps(strategy.draft_revision.overall_take_profit_bps),
    postTrigger: mapPostTriggerToForm(strategy.draft_revision.post_trigger_action),
    quoteAmount: firstLevel ? formatQuote(firstLevel.entry_price, firstLevel.quantity) : "",
    referencePrice: firstLevel?.entry_price ?? "",
    referencePriceMode: "manual",
    symbol: symbolMatchesResult.items[0]?.symbol ?? strategy.symbol,
  };

  const actionButtons = buildActionButtons(lang, strategy.status);

  return (
    <>
      {notice ? (
        <StatusBanner
          description={reason ? `${localize(lang, "原因", "Reason")}: ${reason}` : localize(lang, "最新策略动作已写入工作区。", "The latest strategy action has been written to the workspace.")}
          title={formatNotice(lang, notice)}
          tone={notice.includes("failed") || error ? "warning" : "success"}
        />
      ) : null}
      {step ? <StatusBanner description={`${localize(lang, "阻塞检查", "Blocking step")}: ${describePreflightStep(lang, step)}`} title={localize(lang, "预检提示", "Pre-flight hint")} tone="warning" /> : null}
      {error ? <StatusBanner description={error} title={localize(lang, "策略动作失败", "Strategy action failed")} /> : null}
      {strategyResult.error ? <StatusBanner description={strategyResult.error} title={localize(lang, "策略数据不可用", "Strategy data unavailable")} /> : null}
      {preflightResult.error ? <StatusBanner description={preflightResult.error} title={localize(lang, "预检状态不可用", "Pre-flight status unavailable")} /> : null}
      {analyticsResult.error ? <StatusBanner description={analyticsResult.error} title={localize(lang, "策略分析不可用", "Strategy analytics unavailable")} /> : null}
      {symbolMatchesResult.error ? <StatusBanner description={symbolMatchesResult.error} title={localize(lang, "交易对搜索不可用", "Symbol search unavailable")} /> : null}

      <AppShellSection
        actions={
          <div className="flex items-center gap-2">
            <Link className="inline-flex items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={withLocale(locale, "/app/orders")}>
              {localize(lang, "订单", "Orders")}
            </Link>
            <Link className="inline-flex items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={withLocale(locale, "/app/analytics")}>
              {localize(lang, "分析", "Analytics")}
            </Link>
          </div>
        }
        description={localize(lang, "编辑前先确认预检、运行事件和策略级统计，再决定保存、恢复还是停止。", "Check pre-flight, runtime events, and strategy-level statistics before deciding to save, resume, or stop.")}
        eyebrow={localize(lang, "策略工作台", "Strategy Workspace")}
        title={localize(lang, "策略详情", "Strategy Detail")}
      >
        <div className="content-grid content-grid--metrics">
          {[
            [localize(lang, "交易对", "Symbol"), strategy.symbol],
            [localize(lang, "市场 / 模式", "Market / Mode"), `${describeMarket(lang, strategy.market)} · ${describeMode(lang, strategy.mode)}`],
            [localize(lang, "当前状态", "Current State"), describeStrategyStatus(lang, strategy.status)],
            [localize(lang, "当前持仓", "Current Holdings"), describeHoldings(lang, stats, strategy.runtime.positions)],
          ].map(([label, value]) => (
            <Card key={label}>
              <CardHeader>
                <CardTitle>{value}</CardTitle>
                <CardDescription>{label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>

      <StrategyWorkspaceForm
        editingLocked={strategy.status === "Running"}
        formAction={`/api/user/strategies/${strategy.id}`}
        intentButtons={actionButtons}
        lang={lang}
        searchPath={detailPagePath}
        searchQuery={symbolQuery}
        symbolMatches={symbolMatchesResult.items}
        values={values}
      />

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{localize(lang, "预检清单", "Pre-flight Checklist")}</CardTitle>
            <CardDescription>{localize(lang, "启动前必须逐项通过；失败项会显示当前最先阻塞的步骤。", "Every required step must pass before start; failed rows show the first blocking step.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "item", label: localize(lang, "检查项", "Check") },
                { key: "result", label: localize(lang, "结果", "Result"), align: "right" },
              ]}
              emptyMessage={localize(lang, "暂无预检数据。", "No pre-flight data yet.")}
              rows={(preflight?.steps ?? []).map((row) => ({
                id: row.step,
                item: describePreflightStep(lang, row.step),
                result: <Chip tone={row.status === "Passed" ? "success" : row.status === "Failed" ? "danger" : "info"}>{describePreflightStatus(lang, row.status)}</Chip>,
              }))}
            />
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{localize(lang, "运行事件", "Runtime Events")}</CardTitle>
            <CardDescription>{localize(lang, "这里直接显示暂停、停止、恢复和异常原因，不再只剩下一句泛化报错。", "Pause, stop, resume, and exception reasons stay visible here instead of collapsing into a generic failure banner.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: localize(lang, "时间", "Timestamp") },
                { key: "event", label: localize(lang, "事件", "Event") },
                { key: "detail", label: localize(lang, "详情", "Detail") },
              ]}
              emptyMessage={localize(lang, "暂无运行事件。", "No runtime events yet.")}
              rows={strategy.runtime.events.map((event, index) => ({
                id: `${strategy.id}-event-${index}`,
                at: formatTaipeiDateTime(event.created_at, lang),
                event: describeRuntimeEvent(lang, event.event_type),
                detail: describeRuntimeEventDetail(lang, event.event_type, event.detail),
              }))}
            />
          </CardBody>
        </Card>
      </div>

      <div className="content-grid content-grid--metrics">
        {[
          [localize(lang, "已实现盈亏", "Realized PnL"), stats?.realized_pnl ?? "-"],
          [localize(lang, "未实现盈亏", "Unrealized PnL"), stats?.unrealized_pnl ?? "-"],
          [localize(lang, "手续费", "Fees"), stats?.fees_paid ?? "-"],
          [localize(lang, "资金费", "Funding"), stats?.funding_total ?? "-"],
          [localize(lang, "净利润", "Net PnL"), stats?.net_pnl ?? "-"],
          [localize(lang, "成本基础", "Cost Basis"), stats?.cost_basis ?? strategy.budget],
          [localize(lang, "成交笔数", "Fill Count"), String(stats?.fill_count ?? strategy.runtime.fills.length)],
          [localize(lang, "订单数量", "Order Count"), String(stats?.order_count ?? strategy.runtime.orders.length)],
        ].map(([label, value]) => (
          <Card key={label}>
            <CardHeader>
              <CardTitle>{value}</CardTitle>
              <CardDescription>{label}</CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>
    </>
  );
}

function describeHoldings(
  lang: UiLanguage,
  stats: AnalyticsReport["strategies"][number] | null,
  positions: Array<{ average_entry_price: string; quantity: string }>,
) {
  const live = positions.map((position) => `${position.quantity} @ ${position.average_entry_price}`).join(" | ");
  if (live) {
    return live;
  }
  const items: string[] = [];
  if (stats?.long_position_quantity && stats.long_position_quantity !== "0") {
    items.push(`${localize(lang, "多头", "Long")} ${stats.long_position_quantity} @ ${stats.long_average_entry_price ?? "0"}`);
  }
  if (stats?.short_position_quantity && stats.short_position_quantity !== "0") {
    items.push(`${localize(lang, "空头", "Short")} ${stats.short_position_quantity} @ ${stats.short_average_entry_price ?? "0"}`);
  }
  return items.join(" | ") || stats?.position_quantity || "0";
}

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function localize(lang: UiLanguage, zh: string, en: string) {
  return pickText(lang, zh, en);
}

function withLocale(locale: string, path: string) {
  return `/${locale}${path}`;
}

function mapMarketToForm(value: string): StrategyWorkspaceValues["marketType"] {
  switch (value) {
    case "FuturesUsdM":
      return "usd-m";
    case "FuturesCoinM":
      return "coin-m";
    default:
      return "spot";
  }
}

function mapModeToForm(value: string): StrategyWorkspaceValues["mode"] {
  switch (value) {
    case "SpotBuyOnly":
      return "buy-only";
    case "SpotSellOnly":
      return "sell-only";
    case "FuturesLong":
      return "long";
    case "FuturesShort":
      return "short";
    case "FuturesNeutral":
      return "neutral";
    default:
      return "classic";
  }
}

function mapGenerationToForm(value: string): StrategyWorkspaceValues["generation"] {
  switch (value) {
    case "Geometric":
      return "geometric";
    case "Custom":
      return "custom";
    default:
      return "arithmetic";
  }
}

function mapPostTriggerToForm(value: string): StrategyWorkspaceValues["postTrigger"] {
  return value === "Stop" ? "stop" : "rebuild";
}

function buildActionButtons(lang: UiLanguage, status: string): StrategyWorkspaceIntentButton[] {
  const zh = lang === "zh";
  if (status === "Running") {
    return [
      { label: zh ? "暂停策略" : "Pause Strategy", tone: "outline", value: "pause" },
      { label: zh ? "停止策略" : "Stop Strategy", tone: "outline", value: "stop" },
    ];
  }
  return [
    { label: zh ? "保存修改" : "Save Changes", value: "save" },
    { label: zh ? "运行预检" : "Run Pre-flight", tone: "secondary", value: "preflight" },
    { label: status === "Paused" ? (zh ? "恢复策略" : "Resume Strategy") : (zh ? "启动策略" : "Start Strategy"), value: "start" },
    { label: zh ? "停止策略" : "Stop Strategy", tone: "outline", value: "stop" },
    { label: zh ? "删除策略" : "Delete Strategy", tone: "danger", value: "delete" },
  ];
}

function describeMarket(lang: UiLanguage, value: string) {
  switch (value) {
    case "FuturesUsdM":
      return localize(lang, "U 本位合约", "USD-M Futures");
    case "FuturesCoinM":
      return localize(lang, "币本位合约", "COIN-M Futures");
    default:
      return localize(lang, "现货", "Spot");
  }
}

function describeMode(lang: UiLanguage, value: string) {
  switch (value) {
    case "SpotBuyOnly":
      return localize(lang, "只买", "Buy Only");
    case "SpotSellOnly":
      return localize(lang, "只卖", "Sell Only");
    case "FuturesLong":
      return localize(lang, "做多", "Long");
    case "FuturesShort":
      return localize(lang, "做空", "Short");
    case "FuturesNeutral":
      return localize(lang, "中性", "Neutral");
    default:
      return localize(lang, "经典", "Classic");
  }
}

function describeStrategyStatus(lang: UiLanguage, status: string) {
  switch (status) {
    case "Draft":
      return localize(lang, "草稿", "Draft");
    case "Running":
      return localize(lang, "运行中", "Running");
    case "Paused":
      return localize(lang, "已暂停", "Paused");
    case "ErrorPaused":
      return localize(lang, "异常阻塞", "Blocked");
    case "Stopped":
      return localize(lang, "已停止", "Stopped");
    case "Completed":
      return localize(lang, "已完成", "Completed");
    default:
      return status;
  }
}

function describePreflightStatus(lang: UiLanguage, status: string) {
  switch (status) {
    case "Passed":
      return localize(lang, "通过", "Passed");
    case "Failed":
      return localize(lang, "失败", "Failed");
    case "Skipped":
      return localize(lang, "跳过", "Skipped");
    default:
      return status;
  }
}

function describePreflightStep(lang: UiLanguage, step: string) {
  switch (step) {
    case "membership_status":
      return localize(lang, "会员状态", "Membership Status");
    case "exchange_connection":
      return localize(lang, "交易所连接", "Exchange Connection");
    case "exchange_permissions":
      return localize(lang, "交易权限", "Exchange Permissions");
    case "withdrawal_permission_disabled":
      return localize(lang, "禁提校验", "Withdrawals Disabled");
    case "hedge_mode":
      return localize(lang, "双向持仓", "Hedge Mode");
    case "symbol_support":
      return localize(lang, "交易对支持", "Symbol Support");
    case "filters_and_notional":
      return localize(lang, "过滤器与最小名义", "Filters and Notional");
    case "margin_or_leverage":
      return localize(lang, "保证金或杠杆", "Margin or Leverage");
    case "strategy_conflicts":
      return localize(lang, "策略冲突", "Strategy Conflicts");
    case "balance_or_collateral":
      return localize(lang, "余额或保证金", "Balance or Collateral");
    case "trailing_take_profit":
      return localize(lang, "追踪止盈", "Trailing Take Profit");
    default:
      return step;
  }
}

function describeRuntimeEvent(lang: UiLanguage, eventType: string) {
  switch (eventType) {
    case "strategy_started":
      return localize(lang, "策略已启动", "Strategy Started");
    case "strategy_paused":
      return localize(lang, "策略已暂停", "Strategy Paused");
    case "strategy_stopped":
      return localize(lang, "策略已停止", "Strategy Stopped");
    case "strategy_resumed":
      return localize(lang, "策略已恢复", "Strategy Resumed");
    case "strategy_archived":
      return localize(lang, "策略已归档", "Strategy Archived");
    default:
      return eventType.replace(/_/g, " ");
  }
}

function describeRuntimeEventDetail(
  lang: UiLanguage,
  eventType: string,
  fallback: string,
) {
  switch (eventType) {
    case "strategy_started":
      return localize(lang, "策略已完成启动。", "The strategy finished starting.");
    case "strategy_paused":
      return localize(lang, "策略已暂停，现有持仓会保留。", "The strategy is paused and current holdings remain open.");
    case "strategy_stopped":
      return localize(lang, "策略已停止，系统会等待平仓链路完成。", "The strategy is stopped and waits for the close-out flow to finish.");
    case "strategy_resumed":
      return localize(lang, "策略已按当前账户状态重建并恢复。", "The strategy was rebuilt from the current account state and resumed.");
    case "membership_grace_paused":
      return localize(lang, "会员宽限期结束，系统已自动暂停策略。", "The membership grace period ended and the strategy was auto-paused.");
    case "runtime_error_auto_paused":
      return localize(lang, "运行异常触发自动暂停，请先处理提示原因。", "A runtime error auto-paused the strategy. Resolve the reported reason first.");
    case "overall_take_profit_stop":
      return localize(lang, "整体止盈触发，系统已进入平仓停止流程。", "Overall take profit triggered and the strategy entered close-and-stop.");
    case "overall_take_profit_rebuild":
      return localize(lang, "整体止盈触发，系统将在平仓完成后重建。", "Overall take profit triggered and the strategy will rebuild after closing.");
    case "overall_stop_loss_stop":
      return localize(lang, "整体止损触发，系统已进入平仓停止流程。", "Overall stop loss triggered and the strategy entered close-and-stop.");
    case "overall_stop_loss_rebuild":
      return localize(lang, "整体止损触发，系统将在平仓完成后重建。", "Overall stop loss triggered and the strategy will rebuild after closing.");
    default:
      return fallback;
  }
}

function formatNotice(lang: UiLanguage, value: string) {
  switch (value) {
    case "edits-saved":
      return localize(lang, "修改已保存", "Edits Saved");
    case "preflight-passed":
      return localize(lang, "预检通过", "Pre-flight Passed");
    case "preflight-failed":
      return localize(lang, "预检失败", "Pre-flight Failed");
    case "strategy-paused":
      return localize(lang, "策略已暂停", "Strategy Paused");
    case "strategy-stopped":
      return localize(lang, "策略已停止", "Strategy Stopped");
    case "strategy-started":
      return localize(lang, "策略已启动", "Strategy Started");
    case "strategy-deleted":
      return localize(lang, "策略已删除", "Strategy Deleted");
    case "start-failed":
      return localize(lang, "启动失败", "Start Failed");
    default:
      return value;
  }
}

function formatBps(value: number | null) {
  return value ? (value / 100).toFixed(2) : "";
}

function formatQuote(entryPrice: string, quantity: string) {
  const price = Number.parseFloat(entryPrice);
  const size = Number.parseFloat(quantity);
  if (!Number.isFinite(price) || !Number.isFinite(size)) {
    return "";
  }
  return (price * size).toFixed(2);
}

function computeSpacingPercent(first?: string, second?: string) {
  const firstValue = Number.parseFloat(first ?? "");
  const secondValue = Number.parseFloat(second ?? "");
  if (!Number.isFinite(firstValue) || !Number.isFinite(secondValue) || firstValue === 0) {
    return "1.50";
  }
  return Math.abs(((secondValue - firstValue) / firstValue) * 100).toFixed(2);
}

async function fetchStrategy(strategyId: string, lang: UiLanguage): Promise<{ strategy: BackendStrategy | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { strategy: null, error: localize(lang, "会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { strategy: null, error: localize(lang, "策略工作台暂不可用。", "Strategy workspace is temporarily unavailable.") };
  }
  const payload = (await response.json()) as { items: BackendStrategy[] };
  return { strategy: payload.items.find((item) => item.id === strategyId) ?? null, error: null };
}

async function fetchPreflight(strategyId: string, lang: UiLanguage): Promise<{ preflight: PreflightReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { preflight: null, error: localize(lang, "会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies/${strategyId}/preflight`, {
    method: "POST",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { preflight: null, error: localize(lang, "无法加载预检状态。", "Unable to load pre-flight status.") };
  }
  return { preflight: (await response.json()) as PreflightReport, error: null };
}

async function fetchAnalytics(lang: UiLanguage): Promise<{ analytics: AnalyticsReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { analytics: null, error: localize(lang, "会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/analytics`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { analytics: null, error: localize(lang, "无法加载策略分析。", "Unable to load strategy analytics.") };
  }
  return { analytics: (await response.json()) as AnalyticsReport, error: null };
}

async function fetchSymbolMatches(query: string, lang: UiLanguage): Promise<{ items: SymbolSearchResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken || !query.trim()) {
    return { items: fallbackSymbolMatches(query), error: null };
  }
  const response = await fetch(`${authApiBaseUrl()}/exchange/binance/symbols/search`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ query }),
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: fallbackSymbolMatches(query), error: localize(lang, "当前无法搜索交易对，已回退到常用候选。", "Unable to search symbols right now. Showing common candidates instead.") };
  }
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
