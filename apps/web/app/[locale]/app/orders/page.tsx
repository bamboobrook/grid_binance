import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type AnalyticsReport = {
  account_snapshots: Array<{
    captured_at: string;
    exchange: string;
    fees_paid: string;
    funding_total: string;
  }>;
  exchange_trades: Array<{
    exchange: string;
    fee_amount: string | null;
    fee_asset: string | null;
    price: string;
    quantity: string;
    side: string;
    symbol: string;
    trade_id: string;
    traded_at: string;
  }>;
};

type StrategyListResponse = {
  items: Array<{
    id: string;
    name: string;
    status: string;
    symbol: string;
  }>;
};

type StrategyRuntimeResponse = {
  fills: Array<{
    fill_id: string;
    fill_type: string;
    order_id: string | null;
    price: string;
    quantity: string;
    realized_pnl: string | null;
  }>;
  orders: Array<{
    order_id: string;
    order_type: string;
    price: string | null;
    quantity: string;
    side: string;
    status: string;
  }>;
  strategy_id: string;
};

type OrderRow = {
  id: string;
  orderId: string;
  side: string;
  state: string;
  strategy: string;
  symbol: string;
};

type FillRow = {
  id: string;
  event: string;
  pnl: string;
  symbol: string;
};

export default async function OrdersPage() {
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const results = await Promise.all([fetchAnalytics(), fetchStrategies()]);
  const analytics = results[0];
  const strategies = results[1];
  const runtimes = await fetchStrategyRuntimes(strategies);
  const orderRows = flattenOrders(strategies, runtimes);
  const fillRows = flattenFills(strategies, runtimes);
  const accountSnapshots = analytics?.account_snapshots ?? [];
  const exchangeTrades = analytics?.exchange_trades ?? [];

  return (
    <>
      <StatusBanner
        description={pickText(lang, "订单、成交和交易所侧成交记录统一来自后端运行态与分析接口。", "Orders, fills, and exchange-side trades come directly from backend runtime and analytics data.")}
        title={pickText(lang, "订单状态条", "Orders status strip")}
       
      />
      <AppShellSection
        description={pickText(lang, "这个页面用于核对挂单、成交与交易所侧执行，不离开用户工作台。", "Use this page to reconcile working orders, fills, and exchange executions without leaving the user shell.")}
        eyebrow={pickText(lang, "用户订单", "User orders")}
        title={pickText(lang, "订单与历史", "Orders & History")}
        actions={
          <div className="button-row">
            <a className="button button--ghost" href="/api/user/exports/orders">{pickText(lang, "导出订单 CSV", "Download orders CSV")}</a>
            <a className="button button--ghost" href="/api/user/exports/fills">{pickText(lang, "导出成交 CSV", "Download fills CSV")}</a>
          </div>
        }
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "策略挂单", "Strategy orders")}</CardTitle>
              <CardDescription>{pickText(lang, "每一行都来自真实运行时订单簿。", "Every row is rendered from the live runtime order book.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "orderId", label: pickText(lang, "订单号", "Order ID") },
                  { key: "strategy", label: pickText(lang, "策略", "Strategy") },
                  { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                  { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
                ]}
                rows={orderRows.map((row) => ({
                  id: row.id,
                  orderId: row.orderId,
                  strategy: row.strategy,
                  symbol: row.symbol + " · " + describeSide(lang, row.side),
                  state: <Chip tone={orderTone(row.state)}>{describeOrderState(lang, row.state)}</Chip>,
                }))}
              />
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "成交历史", "Fill history")}</CardTitle>
              <CardDescription>{pickText(lang, "逐笔盈亏来自真实成交，不使用伪造汇总。", "Per-fill PnL comes from actual runtime fills, not fabricated summaries.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "event", label: pickText(lang, "事件", "Event") },
                  { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                  { key: "pnl", label: pickText(lang, "收益", "PnL"), align: "right" },
                ]}
                rows={fillRows.map((row) => ({
                  id: row.id,
                  event: row.event,
                  symbol: row.symbol,
                  pnl: row.pnl,
                }))}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "最近交易所成交", "Recent exchange trades")}</CardTitle>
            <CardDescription>{pickText(lang, "这些记录用于对账 Binance 侧的实际执行。", "These rows help reconcile actual Binance-side executions.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: pickText(lang, "时间", "Timestamp") },
                { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                { key: "detail", label: pickText(lang, "明细", "Detail") },
                { key: "fee", label: pickText(lang, "手续费", "Fee"), align: "right" },
              ]}
              rows={exchangeTrades.map((row) => ({
                id: row.trade_id,
                at: row.traded_at.replace("T", " ").slice(0, 16),
                symbol: row.symbol,
                detail: row.exchange + " · " + describeSide(lang, row.side) + " · " + row.quantity + " @ " + row.price,
                fee: row.fee_amount ? (row.fee_amount + " " + (row.fee_asset ?? "")).trim() : "-",
              }))}
            />
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "账户活动快照", "Exchange account activity")}</CardTitle>
            <CardDescription>{pickText(lang, "账户级分析快照用于复盘成本与资金费。", "Account-level snapshots help review cost and funding drift.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "capturedAt", label: pickText(lang, "时间", "Timestamp") },
                { key: "exchange", label: pickText(lang, "交易所", "Exchange") },
                { key: "detail", label: pickText(lang, "明细", "Detail"), align: "right" },
              ]}
              rows={accountSnapshots.map((row, index) => ({
                id: row.exchange + "-" + index,
                capturedAt: row.captured_at,
                exchange: row.exchange,
                detail: pickText(lang, "手续费 " + row.fees_paid + " | 资金费 " + row.funding_total, "Fees " + row.fees_paid + " | Funding " + row.funding_total),
              }))}
            />
          </CardBody>
        </Card>
      </div>
    </>
  );
}

function describeOrderState(lang: UiLanguage, state: string) {
  switch (state) {
    case "Placed":
      return pickText(lang, "已下单", "Placed");
    case "Working":
      return pickText(lang, "挂单中", "Working");
    case "Canceled":
      return pickText(lang, "已取消", "Canceled");
    case "Filled":
      return pickText(lang, "已成交", "Filled");
    default:
      return state;
  }
}

function describeSide(lang: UiLanguage, side: string) {
  return side === "Buy" ? pickText(lang, "买入", "Buy") : side === "Sell" ? pickText(lang, "卖出", "Sell") : side;
}

function orderTone(state: string) {
  if (state === "Placed" || state === "Working") {
    return "success" as const;
  }
  if (state === "Canceled") {
    return "warning" as const;
  }
  return "info" as const;
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (sessionToken === "") {
    return null;
  }

  const response = await fetch(authApiBaseUrl() + "/analytics", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });

  if (!response.ok) {
    return null;
  }

  return (await response.json()) as AnalyticsReport;
}

async function fetchStrategies(): Promise<StrategyListResponse["items"]> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (sessionToken === "") {
    return [];
  }

  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });

  if (!response.ok) {
    return [];
  }

  return ((await response.json()) as StrategyListResponse).items;
}

async function fetchStrategyRuntimes(strategies: StrategyListResponse["items"]) {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (sessionToken === "") {
    return [] as StrategyRuntimeResponse[];
  }

  const responses = await Promise.all(
    strategies.map(async (strategy) => {
      const response = await fetch(authApiBaseUrl() + "/strategies/" + strategy.id + "/orders", {
        method: "GET",
        headers: { authorization: "Bearer " + sessionToken },
        cache: "no-store",
      });
      if (!response.ok) {
        return null;
      }
      return (await response.json()) as StrategyRuntimeResponse;
    }),
  );

  return responses.filter((item): item is StrategyRuntimeResponse => item !== null);
}

function flattenOrders(
  strategies: StrategyListResponse["items"],
  runtimes: StrategyRuntimeResponse[],
): OrderRow[] {
  return runtimes.flatMap((runtime) => {
    const strategy = strategies.find((item) => item.id === runtime.strategy_id);
    return runtime.orders.map((order) => ({
      id: runtime.strategy_id + "-" + order.order_id,
      orderId: order.order_id,
      side: order.side,
      state: order.status,
      strategy: strategy?.name ?? runtime.strategy_id,
      symbol: strategy?.symbol ?? "-",
    }));
  });
}

function flattenFills(
  strategies: StrategyListResponse["items"],
  runtimes: StrategyRuntimeResponse[],
): FillRow[] {
  return runtimes.flatMap((runtime) => {
    const strategy = strategies.find((item) => item.id === runtime.strategy_id);
    return runtime.fills.map((fill) => ({
      id: fill.fill_id,
      event: fill.fill_type + (fill.order_id ? " · " + fill.order_id : ""),
      pnl: fill.realized_pnl ?? "-",
      symbol: strategy?.symbol ?? "-",
    }));
  });
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
