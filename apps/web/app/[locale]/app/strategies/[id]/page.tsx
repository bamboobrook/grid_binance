import Link from "next/link";
import { cookies } from "next/headers";
import { notFound } from "next/navigation";

import { AppShellSection } from "../../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../../components/ui/card";
import { Chip } from "../../../../../components/ui/chip";
import { DialogFrame } from "../../../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../../../components/ui/form";
import { StatusBanner } from "../../../../../components/ui/status-banner";
import { DataTable } from "../../../../../components/ui/table";

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
    generation: string;
    levels: Array<{
      entry_price: string;
      quantity: string;
      take_profit_bps: number;
      trailing_bps: number | null;
    }>;
    amount_mode?: "Quote" | "Base";
    futures_margin_mode?: "Isolated" | "Cross" | null;
    leverage?: number | null;
    overall_take_profit_bps: number | null;
    overall_stop_loss_bps: number | null;
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

type AnalyticsReport = {
  strategies: Array<{
    average_entry_price: string;
    cost_basis: string;
    current_state: string;
    fees_paid: string;
    fill_count: number;
    funding_total: string;
    net_pnl: string;
    order_count: number;
    position_quantity: string;
    realized_pnl: string;
    strategy_id: string;
    unrealized_pnl: string;
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function bi(zh: string, en: string) {
  return `${zh} / ${en}`;
}

function withLocale(locale: string, path: string) {
  return `/${locale}${path}`;
}

export default async function StrategyDetailPage({ params, searchParams }: PageProps) {
  const { locale, id } = await params;
  const [strategyResult, analyticsResult] = await Promise.all([fetchStrategy(id), fetchAnalytics()]);
  const strategy = strategyResult.strategy;
  const analytics = analyticsResult.analytics;
  const paramsValue = (await searchParams) ?? {};
  const notice = firstValue(paramsValue.notice);
  const symbolQuery = firstValue(paramsValue.symbolQuery) ?? strategy?.symbol ?? "";
  const error = firstValue(paramsValue.error);
  const reason = firstValue(paramsValue.reason);
  const step = firstValue(paramsValue.step);
  const [preflightResult, symbolMatchesResult] = await Promise.all([
    fetchPreflight(id),
    fetchSymbolMatches(firstValue(paramsValue.symbolQuery) ?? strategy?.symbol ?? ""),
  ]);
  const preflight = preflightResult.preflight;

  if (!strategy && !strategyResult.error) {
    notFound();
  }

  if (!strategy) {
    return <StatusBanner title={bi("策略工作台不可用", "Strategy workspace unavailable")} description={strategyResult.error ?? bi("策略工作台暂不可用。", "Strategy workspace is temporarily unavailable.")} />;
  }

  const stats = analytics?.strategies.find((item) => item.strategy_id === strategy.id) ?? null;
  const firstLevel = strategy.draft_revision.levels[0];
  const secondLevel = strategy.draft_revision.levels[1];
  const trailingPercent = firstLevel?.trailing_bps ? (firstLevel.trailing_bps / 100).toFixed(2) : "";
  const holdings = strategy.runtime.positions.map((position) => `${position.quantity} @ ${position.average_entry_price}`).join(" | ") || stats?.position_quantity || "0";
  const levelsJson = JSON.stringify(strategy.draft_revision.levels, null, 2);
  const editorMode = strategy.draft_revision.generation === "Custom" ? "custom" : "batch";
  const amountMode = strategy.draft_revision.amount_mode === "Base" ? "base" : "quote";
  const futuresMarginMode = strategy.draft_revision.futures_margin_mode === "Cross" ? "cross" : "isolated";
  const leverage = strategy.draft_revision.leverage ? String(strategy.draft_revision.leverage) : "5";
  const quoteAmount = firstLevel ? (Number.parseFloat(firstLevel.quantity) * Number.parseFloat(firstLevel.entry_price)).toFixed(2) : "";
  const baseQuantity = firstLevel?.quantity ?? "";
  const referencePrice = firstLevel?.entry_price ?? "";
  const gridCount = String(strategy.draft_revision.levels.length || 0);
  const batchSpacingPercent = computeSpacingPercent(firstLevel?.entry_price, secondLevel?.entry_price);
  const batchTakeProfitPercent = firstLevel ? (firstLevel.take_profit_bps / 100).toFixed(2) : "";
  const detailPagePath = withLocale(locale, `/app/strategies/${strategy.id}`);

  return (
    <>
      {notice ? (
        <StatusBanner
          description={reason ? `${bi("原因", "Reason")}: ${reason}` : bi("最新策略动作已记录到后端工作区。", "The latest strategy action has been recorded in the backend workspace.")}
          title={formatNotice(notice)}
          tone={notice.includes("failed") || error ? "warning" : "success"}
        />
      ) : null}
      {step ? <StatusBanner description={`${bi("阻塞检查", "Blocking step")}: ${describePreflightStep(step)}`} title={bi("预检提示", "Pre-flight hint")} tone="warning" /> : null}
      {error ? <StatusBanner description={error} title={bi("策略动作失败", "Strategy action failed")} /> : null}
      {strategyResult.error ? <StatusBanner description={strategyResult.error} title={bi("策略数据不可用", "Strategy data unavailable")} /> : null}
      {preflightResult.error ? <StatusBanner description={preflightResult.error} title={bi("预检状态不可用", "Pre-flight status unavailable")} /> : null}
      {analyticsResult.error ? <StatusBanner description={analyticsResult.error} title={bi("策略分析不可用", "Strategy analytics unavailable")} /> : null}
      {symbolMatchesResult.error ? <StatusBanner description={symbolMatchesResult.error} title={bi("交易对搜索不可用", "Symbol search unavailable")} /> : null}

      <AppShellSection
        actions={
          <div className="flex items-center gap-2">
            <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 hover:bg-secondary text-foreground transition-colors" href={withLocale(locale, "/app/orders")}>
              {bi("订单", "Orders")}
            </Link>
            <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 hover:bg-secondary text-foreground transition-colors" href={withLocale(locale, "/app/help")}>
              {bi("帮助中心", "Help Center")}
            </Link>
          </div>
        }
        description={bi("在重启或启动前，先核对已保存参数、独立策略统计和实时预检状态。", "Review saved parameters, strategy analytics, and pre-flight status before restarting or launching.")}
        eyebrow={bi("策略工作台", "Strategy workspace")}
        title={bi("策略详情", "Strategy workspace")}
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>{strategy.name}</CardTitle>
              <CardDescription>{strategy.symbol}</CardDescription>
            </CardHeader>
            <CardBody>{describeMarket(strategy.market)}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{describeMode(strategy.mode)}</CardTitle>
              <CardDescription>{bi("模式", "Mode")}</CardDescription>
            </CardHeader>
            <CardBody>{bi("生成方式", "Generation")}: {describeGeneration(strategy.draft_revision.generation)}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{trailingPercent || "-"}%</CardTitle>
              <CardDescription>{bi("追踪止盈", "Trailing take profit")}</CardDescription>
            </CardHeader>
            <CardBody>{bi("仅在可接受 taker 成交费时启用。", "Use only when taker execution fees are acceptable.")}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>
                <Chip tone={strategy.status === "Running" ? "success" : preflight?.ok ? "info" : "warning"}>
                  {describeStrategyStatus(strategy.status)}
                </Chip>
              </CardTitle>
              <CardDescription>{bi("当前状态", "Current state")}</CardDescription>
            </CardHeader>
            <CardBody>{bi("编辑前先暂停，保存后再恢复，运行中不支持热修改。", "Pause before editing, save before restarting, and do not hot-modify while running.")}</CardBody>
          </Card>
        </div>
      </AppShellSection>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>{bi("编辑与生命周期流", "Edit and lifecycle flow")}</CardTitle>
            <CardDescription>{bi("预检与策略控制都会走真实 POST 路由并写回后端。", "Pre-flight and strategy controls are backed by real POST routes against the backend.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <form action={detailPagePath} id="detail-symbol-search-form" method="get" />
            <FormStack action={`/api/user/strategies/${strategy.id}`} method="post">
              <Field label={bi("策略名称", "Strategy name")}>
                <Input defaultValue={strategy.name} name="name" required />
              </Field>
              <Field label={bi("搜索交易对", "Search symbols")} hint={bi("模糊搜索使用已同步的 Binance 元数据。", "Fuzzy search uses synced Binance metadata.")}>
                <div className="flex items-center gap-2">
                  <Input defaultValue={symbolQuery} form="detail-symbol-search-form" name="symbolQuery" />
                  <Button form="detail-symbol-search-form" type="submit">{bi("搜索", "Search")}</Button>
                </div>
              </Field>
              <Field label={bi("交易对", "Symbol")}>
                <Input defaultValue={symbolMatchesResult.items[0]?.symbol ?? strategy.symbol} list="detail-symbol-suggestions" name="symbol" required />
                <datalist id="detail-symbol-suggestions">
                  {symbolMatchesResult.items.map((item) => (
                    <option key={item.symbol} value={item.symbol}>{describeMarket(item.market)} · {item.base_asset}/{item.quote_asset}</option>
                  ))}
                </datalist>
              </Field>
              <Field label={bi("市场类型", "Market type")}>
                <Select defaultValue={mapMarketToForm(strategy.market)} name="marketType">
                  <option value="spot">spot</option>
                  <option value="usd-m">usd-m</option>
                  <option value="coin-m">coin-m</option>
                </Select>
              </Field>
              <Field label={bi("策略模式", "Strategy mode")}>
                <Select defaultValue={mapModeToForm(strategy.mode)} name="mode">
                  <option value="classic">classic</option>
                  <option value="buy-only">buy-only</option>
                  <option value="sell-only">sell-only</option>
                  <option value="long">long</option>
                  <option value="short">short</option>
                  <option value="neutral">neutral</option>
                </Select>
              </Field>
              <Field label={bi("生成方式", "Generation mode")}>
                <Select defaultValue={mapGenerationToForm(strategy.draft_revision.generation)} name="generation">
                  <option value="arithmetic">arithmetic</option>
                  <option value="geometric">geometric</option>
                  <option value="custom">custom</option>
                </Select>
              </Field>
              <Field label={bi("编辑模式", "Editor mode")} hint={bi("batch 会重算阶梯，custom 会保留完整 JSON。", "Batch rebuilds the ladder, while custom keeps the full JSON payload.")}>
                <Select defaultValue={editorMode} name="editorMode">
                  <option value="batch">{bi("批量阶梯生成", "Batch ladder builder")}</option>
                  <option value="custom">{bi("自定义 JSON", "Custom JSON")}</option>
                </Select>
              </Field>
              <Field label={bi("计量模式", "Amount mode")}>
                <Select defaultValue={amountMode} name="amountMode">
                  <option value="quote">{bi("报价资产金额", "Quote amount")}</option>
                  <option value="base">{bi("基础资产数量", "Base quantity")}</option>
                </Select>
              </Field>
              <Field label={bi("合约保证金模式", "Futures margin mode")} hint={bi("仅合约策略使用，现货会忽略该字段。", "Only futures strategies use this field; spot ignores it.")}>
                <Select defaultValue={futuresMarginMode} name="futuresMarginMode">
                  <option value="isolated">{bi("逐仓", "Isolated")}</option>
                  <option value="cross">{bi("全仓", "Cross")}</option>
                </Select>
              </Field>
              <Field label={bi("杠杆", "Leverage")}>
                <Input defaultValue={leverage} inputMode="numeric" name="leverage" />
              </Field>
              <Field label={bi("报价金额 (USDT)", "Quote amount (USDT)")}>
                <Input defaultValue={quoteAmount} inputMode="decimal" name="quoteAmount" />
              </Field>
              <Field label={bi("基础资产数量", "Base asset quantity")}>
                <Input defaultValue={baseQuantity} inputMode="decimal" name="baseQuantity" />
              </Field>
              <Field label={bi("参考价格", "Reference price")}>
                <Input defaultValue={referencePrice} inputMode="decimal" name="referencePrice" />
              </Field>
              <Field label={bi("网格数量", "Grid count")}>
                <Input defaultValue={gridCount} inputMode="numeric" name="gridCount" />
              </Field>
              <Field label={bi("批量间距 (%)", "Batch spacing (%)")}>
                <Input defaultValue={batchSpacingPercent} inputMode="decimal" name="gridSpacingPercent" />
              </Field>
              <Field label={bi("批量止盈 (%)", "Batch take profit (%)")}>
                <Input defaultValue={batchTakeProfitPercent} inputMode="decimal" name="batchTakeProfit" />
              </Field>
              <Field label={bi("追踪止盈 (%)", "Trailing take profit (%)")}>
                <Input defaultValue={trailingPercent} inputMode="decimal" name="batchTrailing" />
              </Field>
              <Field label={bi("整体止盈 (%)", "Overall take profit (%)")}>
                <Input defaultValue={formatBps(strategy.draft_revision.overall_take_profit_bps)} inputMode="decimal" name="overallTakeProfit" />
              </Field>
              <Field label={bi("整体止损 (%)", "Overall stop loss (%)")}>
                <Input defaultValue={formatBps(strategy.draft_revision.overall_stop_loss_bps)} inputMode="decimal" name="overallStopLoss" />
              </Field>
              <Field label={bi("网格 JSON", "Grid levels JSON")} hint={bi("修改前先暂停，保存后重新预检再恢复。", "Pause before editing, save, then rerun pre-flight before restarting.")}>
                <textarea className="ui-input" defaultValue={levelsJson} name="levels_json" rows={10} />
              </Field>
              <Field label={bi("触发后行为", "Post-trigger behavior")}>
                <Select defaultValue={mapPostTriggerToForm(strategy.draft_revision.post_trigger_action)} name="postTrigger">
                  <option value="stop">{bi("执行后停止", "Stop after execution")}</option>
                  <option value="rebuild">{bi("重建后继续", "Rebuild and continue")}</option>
                </Select>
              </Field>
              <ButtonRow>
                <Button name="intent" type="submit" value="save">{bi("保存修改", "Save edits")}</Button>
                <Button name="intent" type="submit" value="preflight">{bi("运行预检", "Run pre-flight")}</Button>
                <Button name="intent" type="submit" value="start">{bi("启动策略", "Start strategy")}</Button>
              </ButtonRow>
              <ButtonRow>
                <Button name="intent" type="submit" value="pause">{bi("暂停策略", "Pause strategy")}</Button>
                <Button name="intent" type="submit" value="stop">{bi("停止策略", "Stop strategy")}</Button>
                <Button name="intent" type="submit" value="delete">{bi("删除策略", "Delete strategy")}</Button>
              </ButtonRow>
            </FormStack>
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{bi("预检清单", "Pre-flight checklist")}</CardTitle>
            <CardDescription>{bi("启动前必须全部通过；失败项会说明具体阻塞点。", "All checks must pass before start; failures explain the exact blocker.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "item", label: bi("检查项", "Check") },
                { key: "result", label: bi("结果", "Result"), align: "right" },
              ]}
              emptyMessage={bi("暂无预检数据。", "No pre-flight data yet.")}
              rows={(preflight?.steps ?? []).map((row) => ({
                id: row.step,
                item: describePreflightStep(row.step),
                result: <Chip tone={row.status === "Passed" ? "success" : row.status === "Failed" ? "danger" : "info"}>{describePreflightStatus(row.status)}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{bi("运行事件", "Runtime events")}</CardTitle>
          <CardDescription>{bi("运行失败与恢复线索会直接展示在这里，而不是只藏在通知里。", "Runtime failures and recovery hints stay visible here instead of hiding only in notifications.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "at", label: bi("时间", "Timestamp") },
              { key: "event", label: bi("事件", "Event") },
              { key: "detail", label: bi("详情", "Detail") },
            ]}
            emptyMessage={bi("暂无运行事件。", "No runtime events yet.")}
            rows={strategy.runtime.events.map((event, index) => ({
              id: `${strategy.id}-event-${index}`,
              at: event.created_at.replace("T", " ").slice(0, 16),
              event: describeRuntimeEvent(event.event_type),
              detail: event.detail,
            }))}
          />
        </CardBody>
      </Card>

      <div className="content-grid content-grid--metrics">
        {[
          [bi("已实现盈亏", "Realized PnL"), stats?.realized_pnl ?? "-"],
          [bi("未实现盈亏", "Unrealized PnL"), stats?.unrealized_pnl ?? "-"],
          [bi("手续费", "Fees"), stats?.fees_paid ?? "-"],
          [bi("资金费", "Funding fees"), stats?.funding_total ?? "-"],
          [bi("净利润", "Net profit"), stats?.net_pnl ?? "-"],
          [bi("成本基础", "Cost basis"), stats?.cost_basis ?? strategy.budget],
          [bi("成交笔数", "Fill count"), String(stats?.fill_count ?? strategy.runtime.fills.length)],
          [bi("订单数量", "Order count"), String(stats?.order_count ?? strategy.runtime.orders.length)],
          [bi("当前持仓", "Current holdings"), holdings],
        ].map(([label, value]) => (
          <Card key={label}>
            <CardHeader>
              <CardTitle>{value}</CardTitle>
              <CardDescription>{label}</CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>{bi("网格阶梯", "Grid ladder")}</CardTitle>
            <CardDescription>{bi("逐层止盈范围会保留在页面上，便于人工复核与导出前核查。", "Per-grid take-profit ranges stay visible for manual review and export readiness.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "level", label: bi("层级", "Level") },
                { key: "range", label: bi("入场价", "Entry") },
                { key: "allocation", label: bi("数量", "Allocation") },
                { key: "tp", label: bi("止盈", "Take profit"), align: "right" },
              ]}
              rows={strategy.draft_revision.levels.map((level, index) => ({
                id: `${strategy.id}-level-${index}`,
                level: `L${index + 1}`,
                range: level.entry_price,
                allocation: level.quantity,
                tp: `${(level.take_profit_bps / 100).toFixed(2)}%`,
              }))}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description={bi("运行中的参数不能热修改；追踪止盈会走 taker 成交，费用可能更高。", "Running strategy parameters cannot be hot-modified. Trailing take profit uses taker execution and may increase fees.")}
          title={bi("运行中参数不可热修改", "Running strategy parameters cannot be hot-modified")}
        />
      </div>
    </>
  );
}

async function fetchStrategy(strategyId: string): Promise<{ strategy: BackendStrategy | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { strategy: null, error: bi("会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { strategy: null, error: bi("策略工作台暂不可用。", "Strategy workspace is temporarily unavailable.") };
  }
  const payload = (await response.json()) as { items: BackendStrategy[] };
  return { strategy: payload.items.find((item) => item.id === strategyId) ?? null, error: null };
}

async function fetchPreflight(strategyId: string): Promise<{ preflight: PreflightReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { preflight: null, error: bi("会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies/${strategyId}/preflight`, {
    method: "POST",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { preflight: null, error: bi("无法加载预检状态。", "Unable to load pre-flight status.") };
  }
  return { preflight: (await response.json()) as PreflightReport, error: null };
}

async function fetchAnalytics(): Promise<{ analytics: AnalyticsReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { analytics: null, error: bi("会话已过期。", "Session expired.") };
  }
  const response = await fetch(`${authApiBaseUrl()}/analytics`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { analytics: null, error: bi("无法加载策略分析。", "Unable to load strategy analytics.") };
  }
  return { analytics: (await response.json()) as AnalyticsReport, error: null };
}

function mapMarketToForm(value: string) {
  switch (value) {
    case "FuturesUsdM":
      return "usd-m";
    case "FuturesCoinM":
      return "coin-m";
    default:
      return "spot";
  }
}

function mapModeToForm(value: string) {
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

function mapGenerationToForm(value: string) {
  switch (value) {
    case "Arithmetic":
      return "arithmetic";
    case "Custom":
      return "custom";
    default:
      return "geometric";
  }
}

function mapPostTriggerToForm(value: string) {
  return value === "Stop" ? "stop" : "rebuild";
}

function describeMarket(value: string) {
  switch (value) {
    case "FuturesUsdM":
      return bi("U 本位合约", "USD-M futures");
    case "FuturesCoinM":
      return bi("币本位合约", "COIN-M futures");
    default:
      return bi("现货", "Spot");
  }
}

function describeMode(value: string) {
  switch (value) {
    case "SpotBuyOnly":
      return bi("只买", "Buy-only");
    case "SpotSellOnly":
      return bi("只卖", "Sell-only");
    case "FuturesLong":
      return bi("做多", "Long");
    case "FuturesShort":
      return bi("做空", "Short");
    case "FuturesNeutral":
      return bi("中性", "Neutral");
    default:
      return bi("经典", "Classic");
  }
}

function describeGeneration(value: string) {
  switch (value) {
    case "Arithmetic":
      return bi("等差", "Arithmetic");
    case "Custom":
      return bi("自定义", "Custom");
    default:
      return bi("等比", "Geometric");
  }
}

function describeStrategyStatus(status: string) {
  switch (status) {
    case "Draft":
      return bi("草稿", "Draft");
    case "Running":
      return bi("运行中", "Running");
    case "Paused":
      return bi("已暂停", "Paused");
    case "ErrorPaused":
      return bi("异常阻塞", "Blocked");
    case "Stopped":
      return bi("已停止", "Stopped");
    default:
      return status;
  }
}

function describePreflightStatus(status: string) {
  switch (status) {
    case "Passed":
      return bi("通过", "Passed");
    case "Failed":
      return bi("失败", "Failed");
    case "Skipped":
      return bi("跳过", "Skipped");
    default:
      return status;
  }
}

function describePreflightStep(step: string) {
  switch (step) {
    case "membership_status":
      return bi("会员状态", "Membership status");
    case "exchange_connection":
      return bi("交易所连接", "Exchange connection");
    case "exchange_permissions":
      return bi("交易权限", "Exchange permissions");
    case "withdrawal_permission_disabled":
      return bi("禁提校验", "Withdrawals disabled");
    case "hedge_mode":
      return bi("双向持仓", "Hedge mode");
    case "symbol_support":
      return bi("交易对支持", "Symbol support");
    case "filters_and_notional":
      return bi("过滤器与最小名义", "Filters and notional");
    case "margin_or_leverage":
      return bi("保证金或杠杆", "Margin or leverage");
    case "strategy_conflicts":
      return bi("策略冲突", "Strategy conflicts");
    case "balance_or_collateral":
      return bi("余额或保证金", "Balance or collateral");
    default:
      return step;
  }
}

function describeRuntimeEvent(eventType: string) {
  switch (eventType) {
    case "strategy_started":
      return bi("策略已启动", "Strategy started");
    case "strategy_paused":
      return bi("策略已暂停", "Strategy paused");
    case "strategy_stopped":
      return bi("策略已停止", "Strategy stopped");
    case "strategy_resumed":
      return bi("策略已恢复", "Strategy resumed");
    case "strategy_saved":
      return bi("策略已保存", "Strategy saved");
    default:
      return eventType.replace(/_/g, " ");
  }
}

function computeSpacingPercent(first?: string, second?: string) {
  const firstValue = Number.parseFloat(first ?? "");
  const secondValue = Number.parseFloat(second ?? "");
  if (!Number.isFinite(firstValue) || !Number.isFinite(secondValue) || firstValue === 0) {
    return "1.50";
  }
  return Math.abs(((secondValue - firstValue) / firstValue) * 100).toFixed(2);
}

function formatNotice(value: string) {
  switch (value) {
    case "template-applied":
      return bi("模板已应用", "Template applied");
    case "edits-saved":
      return bi("修改已保存", "Edits saved");
    case "preflight-passed":
      return bi("预检通过", "Pre-flight passed");
    case "preflight-failed":
      return bi("预检失败", "Pre-flight failed");
    case "strategy-paused":
      return bi("策略已暂停", "Strategy paused");
    case "strategy-stopped":
      return bi("策略已停止", "Strategy stopped");
    case "strategy-started":
      return bi("策略已启动", "Strategy started");
    case "start-failed":
      return bi("启动失败", "Start failed");
    default:
      return value;
  }
}

function formatBps(value: number | null) {
  return value ? (value / 100).toFixed(2) : "";
}

async function fetchSymbolMatches(query: string): Promise<{ items: SymbolSearchResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken || !query.trim()) {
    return { items: [], error: null };
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
    return { items: [], error: bi("当前无法搜索交易对。", "Unable to search symbols right now.") };
  }
  return { items: ((await response.json()) as SymbolSearchResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
