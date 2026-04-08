import Link from "next/link";
import { notFound } from "next/navigation";
import { cookies } from "next/headers";

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
  items: Array<{ base_asset: string; market: string; quote_asset: string; symbol: string }> ;
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
  const [preflightResult, symbolMatchesResult] = await Promise.all([fetchPreflight(id), fetchSymbolMatches(firstValue(paramsValue.symbolQuery) ?? strategy?.symbol ?? "")]);
  const preflight = preflightResult.preflight;

  if (!strategy && !strategyResult.error) {
    notFound();
  }

  if (!strategy) {
    return (
      <>
        <StatusBanner title="Strategy workspace unavailable" description={strategyResult.error ?? "Strategy workspace is temporarily unavailable."} />
      </>
    );
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

  return (
    <>
      {notice ? (
        <StatusBanner
          description={reason ?? "The latest strategy action has been recorded in the backend workspace."}
          title={formatNotice(notice)}
          tone={notice.includes("failed") || error ? "warning" : "success"}
        />
      ) : null}
      {error ? <StatusBanner description={error} title="Strategy action failed" /> : null}
      {strategyResult.error ? <StatusBanner description={strategyResult.error} title={locale === "zh" ? "策略数据不可用" : "Strategy data unavailable"} /> : null}
      {preflightResult.error ? <StatusBanner description={preflightResult.error} title={locale === "zh" ? "预检状态不可用" : "Pre-flight status unavailable"} /> : null}
      {analyticsResult.error ? <StatusBanner description={analyticsResult.error} title={locale === "zh" ? "策略统计不可用" : "Strategy analytics unavailable"} /> : null}
      {symbolMatchesResult.error ? <StatusBanner description={symbolMatchesResult.error} title={locale === "zh" ? "交易对搜索不可用" : "Symbol search unavailable"} /> : null}
      <AppShellSection
        actions={
          <div className="flex items-center gap-2">
            <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 hover:bg-secondary text-foreground transition-colors" href={`/${locale}/app/orders`}>
              {locale === "zh" ? "订单" : "Orders"}
            </Link>
            <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 hover:bg-secondary text-foreground transition-colors" href={`/${locale}/app/help`}>
              {locale === "zh" ? "帮助中心" : "Help Center"}
            </Link>
          </div>
        }
        description={locale === "zh" ? "重启或启动前，在这里核对已保存参数、独立策略统计和预检状态。" : "Review saved parameters, independent strategy statistics, and pre-flight status before restarting or launching."}
        eyebrow={locale === "zh" ? "策略工作区" : "Strategy workspace"}
        title={locale === "zh" ? "策略工作区" : "Strategy Workspace"}
      >
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 mb-6">
          <Card>
            <CardHeader>
              <CardTitle>{strategy.name}</CardTitle>
              <CardDescription>{strategy.symbol}</CardDescription>
            </CardHeader>
            <CardBody>{strategy.market}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{strategy.mode}</CardTitle>
              <CardDescription>{locale === "zh" ? "模式" : "Mode"}</CardDescription>
            </CardHeader>
            <CardBody>{locale === "zh" ? "生成方式" : "Generation"}: {strategy.draft_revision.generation}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{trailingPercent || "-"}%</CardTitle>
              <CardDescription>{locale === "zh" ? "追踪止盈" : "Trailing take profit"}</CardDescription>
            </CardHeader>
            <CardBody>{locale === "zh" ? "只有在能接受 taker 手续费时才建议使用。" : "Use only when taker execution fee tradeoff is acceptable."}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>
                <Chip tone={strategy.status === "Running" ? "success" : preflight?.ok ? "info" : "warning"}>
                  {describeStrategyStatus(strategy.status)}
                </Chip>
              </CardTitle>
              <CardDescription>{locale === "zh" ? "当前状态" : "Current state"}</CardDescription>
            </CardHeader>
            <CardBody>{locale === "zh" ? "修改前先暂停，重启前先保存，运行中不支持热修改。" : "Pause first to edit, save before restart, no hot-modify while running."}</CardBody>
          </Card>
        </div>
      </AppShellSection>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>{locale === "zh" ? "编辑与生命周期" : "Edit and lifecycle flow"}</CardTitle>
            <CardDescription>{locale === "zh" ? "预检与策略控制都是真实后端生命周期动作。" : "Pre-flight and strategy controls are POST-backed lifecycle steps against the real backend."}</CardDescription>
          </CardHeader>
          <CardBody>
            <form action={`/${locale}/app/strategies/${strategy.id}`} id="detail-symbol-search-form" method="get" />
            <FormStack action={`/api/user/strategies/${strategy.id}`} method="post">
              <Field label={locale === "zh" ? "策略名称" : "Strategy name"}>
                <Input defaultValue={strategy.name} name="name" required />
              </Field>
              <Field label={locale === "zh" ? "搜索交易对" : "Search symbols"} hint={locale === "zh" ? "交易对搜索会使用同步后的币安元数据做模糊匹配。" : "Symbol search uses synced Binance metadata for fuzzy matching."}>
                <div className="flex items-center gap-2">
                  <Input defaultValue={symbolQuery} form="detail-symbol-search-form" name="symbolQuery" />
                  <Button form="detail-symbol-search-form" type="submit">{locale === "zh" ? "搜索交易对" : "Search symbols"}</Button>
                </div>
              </Field>
              <Field label={locale === "zh" ? "交易对" : "Symbol"}>
                <Input defaultValue={symbolMatchesResult.items[0]?.symbol ?? strategy.symbol} list="detail-symbol-suggestions" name="symbol" required />
                <datalist id="detail-symbol-suggestions">
                  {symbolMatchesResult.items.map((item) => (
                    <option key={item.symbol} value={item.symbol}>{item.market} · {item.base_asset}/{item.quote_asset}</option>
                  ))}
                </datalist>
              </Field>
              <Field label={locale === "zh" ? "市场类型" : "Market type"}>
                <Select defaultValue={mapMarketToForm(strategy.market)} name="marketType">
                  <option value="spot">spot</option>
                  <option value="usd-m">usd-m</option>
                  <option value="coin-m">coin-m</option>
                </Select>
              </Field>
              <Field label={locale === "zh" ? "策略模式" : "Strategy mode"}>
                <Select defaultValue={mapModeToForm(strategy.mode)} name="mode">
                  <option value="classic">classic</option>
                  <option value="buy-only">buy-only</option>
                  <option value="sell-only">sell-only</option>
                  <option value="long">long</option>
                  <option value="short">short</option>
                  <option value="neutral">neutral</option>
                </Select>
              </Field>
              <Field label="Generation mode">
                <Select defaultValue={mapGenerationToForm(strategy.draft_revision.generation)} name="generation">
                  <option value="arithmetic">arithmetic</option>
                  <option value="geometric">geometric</option>
                  <option value="custom">custom</option>
                </Select>
              </Field>
              <Field label="Editor mode" hint="Batch mode rewrites the ladder from the inputs below. Custom JSON keeps every grid fully manual.">
                <Select defaultValue={editorMode} name="editorMode">
                  <option value="batch">Batch ladder builder</option>
                  <option value="custom">Custom JSON</option>
                </Select>
              </Field>
              <Field label="Amount mode">
                <Select defaultValue={amountMode} name="amountMode">
                  <option value="quote">Quote amount</option>
                  <option value="base">Base asset quantity</option>
                </Select>
              </Field>
              <Field label="Futures margin mode" hint="Required for futures strategies. Spot strategies ignore this setting.">
                <Select defaultValue={futuresMarginMode} name="futuresMarginMode">
                  <option value="isolated">Isolated</option>
                  <option value="cross">Cross</option>
                </Select>
              </Field>
              <Field label="Leverage">
                <Input defaultValue={leverage} inputMode="numeric" name="leverage" />
              </Field>
              <Field label="Quote amount (USDT)">
                <Input defaultValue={quoteAmount} inputMode="decimal" name="quoteAmount" />
              </Field>
              <Field label="Base asset quantity">
                <Input defaultValue={baseQuantity} inputMode="decimal" name="baseQuantity" />
              </Field>
              <Field label="Reference price">
                <Input defaultValue={referencePrice} inputMode="decimal" name="referencePrice" />
              </Field>
              <Field label="Grid count">
                <Input defaultValue={gridCount} inputMode="numeric" name="gridCount" />
              </Field>
              <Field label="Batch spacing (%)">
                <Input defaultValue={batchSpacingPercent} inputMode="decimal" name="gridSpacingPercent" />
              </Field>
              <Field label="Batch take profit (%)">
                <Input defaultValue={batchTakeProfitPercent} inputMode="decimal" name="batchTakeProfit" />
              </Field>
              <Field label="Trailing take profit (%)">
                <Input defaultValue={trailingPercent} inputMode="decimal" name="batchTrailing" />
              </Field>
              <Field label="Overall take profit (%)">
                <Input defaultValue={formatBps(strategy.draft_revision.overall_take_profit_bps)} inputMode="decimal" name="overallTakeProfit" />
              </Field>
              <Field label="Overall stop loss (%)">
                <Input defaultValue={formatBps(strategy.draft_revision.overall_stop_loss_bps)} inputMode="decimal" name="overallStopLoss" />
              </Field>
              <Field label="Grid levels JSON" hint="Pause before editing. Save the JSON, then rerun pre-flight before restart. Use this for fully custom per-grid overrides.">
                <textarea className="ui-input" defaultValue={levelsJson} name="levels_json" rows={10} />
              </Field>
              <Field label="Post-trigger behavior">
                <Select defaultValue={mapPostTriggerToForm(strategy.draft_revision.post_trigger_action)} name="postTrigger">
                  <option value="stop">Stop after execution</option>
                  <option value="rebuild">Rebuild and continue</option>
                </Select>
              </Field>
              <ButtonRow>
                <Button name="intent" type="submit" value="save">
                  Save edits
                </Button>
                <Button name="intent" type="submit" value="preflight">
                  Run pre-flight
                </Button>
                <Button name="intent" type="submit" value="start">
                  Start strategy
                </Button>
              </ButtonRow>
              <ButtonRow>
                <Button name="intent" type="submit" value="pause">
                  Pause strategy
                </Button>
                <Button name="intent" type="submit" value="stop">
                  Stop strategy
                </Button>
                <Button name="intent" type="submit" value="delete">
                  Delete strategy
                </Button>
              </ButtonRow>
            </FormStack>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Pre-flight checklist</CardTitle>
            <CardDescription>Start requires all checks to pass and any failures explain the exact blocker.</CardDescription>
          </CardHeader>
          <CardBody>
            <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
                <DataTable
              columns={[
                { key: "item", label: "Check" },
                { key: "result", label: "Result", align: "right" },
              ]}
              rows={(preflight?.steps ?? []).map((row) => ({
                id: row.step,
                item: row.step,
                result: <Chip tone={row.status === "Passed" ? "success" : row.status === "Failed" ? "danger" : "info"}>{row.status}</Chip>,
              }))}
            />
              </div>
          </CardBody>
        </Card>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Runtime events</CardTitle>
          <CardDescription>Runtime failures and recovery hints stay visible here instead of being hidden only in notifications.</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
                <DataTable
            columns={[
              { key: "at", label: "Timestamp" },
              { key: "event", label: "Event" },
              { key: "detail", label: "Detail" },
            ]}
            rows={strategy.runtime.events.map((event, index) => ({
              id: strategy.id + "-event-" + index,
              at: event.created_at.replace("T", " ").slice(0, 16),
              event: event.event_type,
              detail: event.detail,
            }))}
          />
              </div>
        </CardBody>
      </Card>
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 mb-6">
        {[
          ["Realized PnL", stats?.realized_pnl ?? "-"],
          ["Unrealized PnL", stats?.unrealized_pnl ?? "-"],
          ["Fees", stats?.fees_paid ?? "-"],
          ["Funding fees", stats?.funding_total ?? "-"],
          ["Net profit", stats?.net_pnl ?? "-"],
          ["Cost basis", stats?.cost_basis ?? strategy.budget],
          ["Fill count", String(stats?.fill_count ?? strategy.runtime.fills.length)],
          ["Order count", String(stats?.order_count ?? strategy.runtime.orders.length)],
          ["Current holdings", holdings],
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
            <CardTitle>Grid ladder</CardTitle>
            <CardDescription>Per-grid take-profit ranges stay visible for manual review and export readiness.</CardDescription>
          </CardHeader>
          <CardBody>
            <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
                <DataTable
              columns={[
                { key: "level", label: "Level" },
                { key: "range", label: "Entry" },
                { key: "allocation", label: "Allocation" },
                { key: "tp", label: "Take profit", align: "right" },
              ]}
              rows={strategy.draft_revision.levels.map((level, index) => ({
                id: `${strategy.id}-level-${index}`,
                level: `L${index + 1}`,
                range: level.entry_price,
                allocation: level.quantity,
                tp: `${(level.take_profit_bps / 100).toFixed(2)}%`,
              }))}
            />
              </div>
          </CardBody>
        </Card>
        <DialogFrame
          description="Running strategy parameters cannot be hot-modified. Trailing take profit uses taker execution and may increase fees."
          title="Running strategy parameters cannot be hot-modified"
         
        />
      </div>
    </>
  );
}

async function fetchStrategy(strategyId: string): Promise<{ strategy: BackendStrategy | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { strategy: null, error: "Session expired." };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { strategy: null, error: "Strategy workspace is temporarily unavailable." };
  }
  const payload = (await response.json()) as { items: BackendStrategy[] };
  return { strategy: payload.items.find((item) => item.id === strategyId) ?? null, error: null };
}

async function fetchPreflight(strategyId: string): Promise<{ preflight: PreflightReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { preflight: null, error: "Session expired." };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies/${strategyId}/preflight`, {
    method: "POST",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { preflight: null, error: "Unable to load pre-flight status." };
  }
  return { preflight: (await response.json()) as PreflightReport, error: null };
}

async function fetchAnalytics(): Promise<{ analytics: AnalyticsReport | null; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { analytics: null, error: "Session expired." };
  }
  const response = await fetch(`${authApiBaseUrl()}/analytics`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { analytics: null, error: "Unable to load strategy analytics." };
  }
  return { analytics: (await response.json()) as AnalyticsReport, error: null };
}

function mapMarketToForm(value: string) {
  switch (value) {
    case "FuturesUsdM": return "usd-m";
    case "FuturesCoinM": return "coin-m";
    default: return "spot";
  }
}

function mapModeToForm(value: string) {
  switch (value) {
    case "SpotBuyOnly": return "buy-only";
    case "SpotSellOnly": return "sell-only";
    case "FuturesLong": return "long";
    case "FuturesShort": return "short";
    case "FuturesNeutral": return "neutral";
    default: return "classic";
  }
}

function mapGenerationToForm(value: string) {
  switch (value) {
    case "Arithmetic": return "arithmetic";
    case "Custom": return "custom";
    default: return "geometric";
  }
}

function mapPostTriggerToForm(value: string) {
  return value === "Stop" ? "stop" : "rebuild";
}

function describeStrategyStatus(status: string) {
  switch (status) {
    case "Draft":
      return "Draft";
    case "Running":
      return "Running";
    case "Paused":
      return "Paused";
    case "ErrorPaused":
      return "Blocked";
    case "Stopped":
      return "Stopped";
    default:
      return status;
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
  const parts = value.split("-");
  if (parts.length === 0) {
    return value;
  }
  const first = parts[0] === "preflight" ? "Pre-flight" : parts[0].charAt(0).toUpperCase() + parts[0].slice(1);
  return [first, ...parts.slice(1)].join(" ");
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
    return { items: [], error: "Unable to search symbols right now." };
  }
  return { items: ((await response.json()) as SymbolSearchResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
