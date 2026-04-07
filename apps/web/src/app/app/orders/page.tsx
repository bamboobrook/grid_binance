import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

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
  const [analytics, strategies] = await Promise.all([fetchAnalytics(), fetchStrategies()]);
  const runtimes = await fetchStrategyRuntimes(strategies);
  const orderRows = flattenOrders(strategies, runtimes);
  const fillRows = flattenFills(strategies, runtimes);
  const accountSnapshots = analytics?.account_snapshots ?? [];
  const exchangeTrades = analytics?.exchange_trades ?? [];

  return (
    <>
      <StatusBanner
        description="Orders, fill history, and recent exchange-side trades now come from backend runtime plus analytics data instead of route-local placeholders."
        title="Orders and history"
        tone="info"
      />
      <AppShellSection
        description="Use this route to review working orders, runtime fills, and recent exchange trades without leaving the user shell."
        eyebrow="User orders"
        title="Orders & History"
        actions={<div className="button-row"><a className="button button--ghost" href="/api/user/exports/orders">Download orders CSV</a><a className="button button--ghost" href="/api/user/exports/fills">Download fills CSV</a></div>}
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Strategy orders</CardTitle>
              <CardDescription>Order rows are rendered from the real runtime order book for each strategy.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "orderId", label: "Order ID" },
                  { key: "strategy", label: "Strategy" },
                  { key: "symbol", label: "Symbol" },
                  { key: "state", label: "State", align: "right" },
                ]}
                rows={orderRows.map((row) => ({
                  id: row.id,
                  orderId: row.orderId,
                  strategy: row.strategy,
                  symbol: `${row.symbol} · ${row.side}`,
                  state: <Chip tone={row.state === "Placed" || row.state === "Working" ? "success" : row.state === "Canceled" ? "warning" : "info"}>{row.state}</Chip>,
                }))}
              />
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Fill history</CardTitle>
              <CardDescription>Per-fill profit context comes from runtime fills, not fabricated summaries.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "event", label: "Event" },
                  { key: "symbol", label: "Symbol" },
                  { key: "pnl", label: "PnL", align: "right" },
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
            <CardTitle>Recent exchange trades</CardTitle>
            <CardDescription>These rows come from persisted exchange trade history so users can reconcile Binance-side executions.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: "Timestamp" },
                { key: "symbol", label: "Symbol" },
                { key: "detail", label: "Detail" },
                { key: "fee", label: "Fee", align: "right" },
              ]}
              rows={exchangeTrades.map((row) => ({
                id: row.trade_id,
                at: row.traded_at.replace("T", " ").slice(0, 16),
                symbol: row.symbol,
                detail: `${row.exchange} · ${row.side} · ${row.quantity} @ ${row.price}`,
                fee: row.fee_amount ? `${row.fee_amount} ${row.fee_asset ?? ""}`.trim() : "-",
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Exchange account activity</CardTitle>
            <CardDescription>Account-level analytics snapshots help reconciliation and cost review.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "capturedAt", label: "Timestamp" },
                { key: "exchange", label: "Exchange" },
                { key: "detail", label: "Detail", align: "right" },
              ]}
              rows={accountSnapshots.map((row, index) => ({
                id: `${row.exchange}-${index}`,
                capturedAt: row.captured_at,
                exchange: row.exchange,
                detail: `Fees ${row.fees_paid} | Funding ${row.funding_total}`,
              }))}
            />
          </CardBody>
        </Card>
      </div>
    </>
  );
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }

  const response = await fetch(`${authApiBaseUrl()}/analytics`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
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
  if (!sessionToken) {
    return [];
  }

  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
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
  if (!sessionToken) {
    return [] as StrategyRuntimeResponse[];
  }

  const responses = await Promise.all(
    strategies.map(async (strategy) => {
      const response = await fetch(`${authApiBaseUrl()}/strategies/${strategy.id}/orders`, {
        method: "GET",
        headers: { authorization: `Bearer ${sessionToken}` },
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
      id: `${runtime.strategy_id}-${order.order_id}`,
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
      event: `${fill.fill_type}${fill.order_id ? ` · ${fill.order_id}` : ""}`,
      pnl: fill.realized_pnl ?? "-",
      symbol: strategy?.symbol ?? "-",
    }));
  });
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
