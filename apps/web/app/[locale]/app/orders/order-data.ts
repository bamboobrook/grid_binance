import "server-only";

import { cookies } from "next/headers";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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

export type AccountSnapshotRow = AnalyticsReport["account_snapshots"][number];
export type ExchangeTradeRow = AnalyticsReport["exchange_trades"][number];

export type OrderRow = {
  detail: string;
  id: string;
  orderId: string;
  side: string;
  state: string;
  strategy: string;
  symbol: string;
};

export type FillRow = {
  detail: string;
  id: string;
  event: string;
  pnl: string;
  symbol: string;
};

export type OrdersData = {
  accountSnapshots: AccountSnapshotRow[];
  exchangeTrades: ExchangeTradeRow[];
  fillRows: FillRow[];
  orderRows: OrderRow[];
};

export async function getOrdersData(lang: UiLanguage): Promise<OrdersData> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  const previewMode = process.env.NEXT_PUBLIC_UI_PREVIEW === "1";
  const [analytics, strategies] = sessionToken
    ? await Promise.all([fetchAnalytics(sessionToken), fetchStrategies(sessionToken)])
    : [null, [] as StrategyListResponse["items"]];
  const runtimes = sessionToken ? await fetchStrategyRuntimes(sessionToken, strategies.map((s) => s.id)) : [];
  const liveOrderRows = flattenOrders(lang, strategies, runtimes);
  const liveFillRows = flattenFills(lang, strategies, runtimes);

  return {
    accountSnapshots: analytics?.account_snapshots?.length ? analytics.account_snapshots : previewMode ? previewAccountSnapshots() : [],
    exchangeTrades: analytics?.exchange_trades?.length ? analytics.exchange_trades : previewMode ? previewExchangeTrades() : [],
    fillRows: liveFillRows.length > 0 ? liveFillRows : previewMode ? previewFillRows(lang) : [],
    orderRows: liveOrderRows.length > 0 ? liveOrderRows : previewMode ? previewOrderRows(lang) : [],
  };
}

export function describeOrderState(lang: UiLanguage, state: string) {
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

export function describeSide(lang: UiLanguage, side: string) {
  return side === "Buy" ? pickText(lang, "买入", "Buy") : side === "Sell" ? pickText(lang, "卖出", "Sell") : side;
}

export function orderTone(state: string) {
  if (state === "Placed" || state === "Working") {
    return "success" as const;
  }
  if (state === "Canceled") {
    return "warning" as const;
  }
  return "info" as const;
}

async function fetchAnalytics(sessionToken: string): Promise<AnalyticsReport | null> {
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

async function fetchStrategies(sessionToken: string): Promise<StrategyListResponse["items"]> {
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

async function fetchStrategyRuntimes(sessionToken: string, strategyIds: string[]) {
  if (strategyIds.length === 0) {
    return [] as StrategyRuntimeResponse[];
  }

  const response = await fetch(
    authApiBaseUrl() + "/strategies/batch/runtimes?ids=" + strategyIds.join(","),
    {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    }
  );
  if (!response.ok) {
    return [] as StrategyRuntimeResponse[];
  }
  const data = (await response.json()) as { items: StrategyRuntimeResponse[] };
  return data.items ?? [];
}

function flattenOrders(
  lang: UiLanguage,
  strategies: StrategyListResponse["items"],
  runtimes: StrategyRuntimeResponse[],
): OrderRow[] {
  return runtimes.flatMap((runtime) => {
    const strategy = strategies.find((item) => item.id === runtime.strategy_id);
    return runtime.orders.map((order) => ({
      detail: [
        strategy?.symbol ?? "-",
        describeSide(lang, order.side),
        order.quantity,
        order.price ? "@ " + order.price : pickText(lang, "市价", "Market"),
      ].join(" · "),
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
  lang: UiLanguage,
  strategies: StrategyListResponse["items"],
  runtimes: StrategyRuntimeResponse[],
): FillRow[] {
  return runtimes.flatMap((runtime) => {
    const strategy = strategies.find((item) => item.id === runtime.strategy_id);
    return runtime.fills.map((fill) => ({
      detail: fill.quantity + " @ " + fill.price,
      event: describeFillType(lang, fill.fill_type) + (fill.order_id ? " · " + fill.order_id : ""),
      id: fill.fill_id,
      pnl: fill.realized_pnl ?? "-",
      symbol: strategy?.symbol ?? "-",
    }));
  });
}

function describeFillType(lang: UiLanguage, fillType: string) {
  switch (fillType) {
    case "GridBuy":
      return pickText(lang, "网格买入", "Grid buy");
    case "GridSell":
      return pickText(lang, "网格卖出", "Grid sell");
    case "DcaBuy":
      return pickText(lang, "补仓买入", "DCA buy");
    case "TakeProfit":
      return pickText(lang, "止盈成交", "Take profit");
    default:
      return fillType;
  }
}

function previewOrderRows(lang: UiLanguage): OrderRow[] {
  return [
    {
      detail: pickText(lang, "BTCUSDT · 买入 · 0.006 BTC @ 86,420.00", "BTCUSDT · Buy · 0.006 BTC @ 86,420.00"),
      id: "preview-order-btc-1",
      orderId: "GB-BTC-1208",
      side: "Buy",
      state: "Working",
      strategy: pickText(lang, "BTC 稳健网格", "BTC steady grid"),
      symbol: "BTCUSDT",
    },
    {
      detail: pickText(lang, "BTCUSDT · 卖出 · 0.004 BTC @ 88,160.00", "BTCUSDT · Sell · 0.004 BTC @ 88,160.00"),
      id: "preview-order-btc-2",
      orderId: "GB-BTC-1214",
      side: "Sell",
      state: "Placed",
      strategy: pickText(lang, "BTC 稳健网格", "BTC steady grid"),
      symbol: "BTCUSDT",
    },
    {
      detail: pickText(lang, "ETHUSDT · 买入 · 0.18 ETH @ 3,412.50", "ETHUSDT · Buy · 0.18 ETH @ 3,412.50"),
      id: "preview-order-eth-1",
      orderId: "GB-ETH-0603",
      side: "Buy",
      state: "Working",
      strategy: pickText(lang, "ETH 合约小额试跑", "ETH small futures test"),
      symbol: "ETHUSDT",
    },
    {
      detail: pickText(lang, "SOLUSDT · 卖出 · 12.5 SOL @ 151.80", "SOLUSDT · Sell · 12.5 SOL @ 151.80"),
      id: "preview-order-sol-1",
      orderId: "MT-SOL-0327",
      side: "Sell",
      state: "Canceled",
      strategy: pickText(lang, "SOL 马丁观察仓", "SOL DCA watch bot"),
      symbol: "SOLUSDT",
    },
    {
      detail: pickText(lang, "BNBUSDT · 买入 · 0.9 BNB @ 648.20", "BNBUSDT · Buy · 0.9 BNB @ 648.20"),
      id: "preview-order-bnb-1",
      orderId: "GB-BNB-0091",
      side: "Buy",
      state: "Working",
      strategy: pickText(lang, "BNB 新手模板草稿", "BNB starter template"),
      symbol: "BNBUSDT",
    },
    {
      detail: pickText(lang, "BTCUSDT · 买入 · 0.003 BTC @ 85,760.00", "BTCUSDT · Buy · 0.003 BTC @ 85,760.00"),
      id: "preview-order-btc-3",
      orderId: "GB-BTC-1218",
      side: "Buy",
      state: "Working",
      strategy: pickText(lang, "BTC 稳健网格", "BTC steady grid"),
      symbol: "BTCUSDT",
    },
    {
      detail: pickText(lang, "ETHUSDT · 卖出 · 0.12 ETH @ 3,520.00", "ETHUSDT · Sell · 0.12 ETH @ 3,520.00"),
      id: "preview-order-eth-2",
      orderId: "GB-ETH-0610",
      side: "Sell",
      state: "Placed",
      strategy: pickText(lang, "ETH 合约小额试跑", "ETH small futures test"),
      symbol: "ETHUSDT",
    },
  ];
}

function previewFillRows(lang: UiLanguage): FillRow[] {
  return [
    {
      detail: pickText(lang, "0.005 BTC @ 87,240.00", "0.005 BTC @ 87,240.00"),
      event: pickText(lang, "网格卖出 · GB-BTC-1198", "Grid sell · GB-BTC-1198"),
      id: "preview-fill-btc-1",
      pnl: "+12.48 USDT",
      symbol: "BTCUSDT",
    },
    {
      detail: pickText(lang, "0.22 ETH @ 3,388.20", "0.22 ETH @ 3,388.20"),
      event: pickText(lang, "补仓买入 · GB-ETH-0599", "Add buy · GB-ETH-0599"),
      id: "preview-fill-eth-1",
      pnl: "-",
      symbol: "ETHUSDT",
    },
    {
      detail: pickText(lang, "8.4 SOL @ 148.60", "8.4 SOL @ 148.60"),
      event: pickText(lang, "马丁减仓 · MT-SOL-0319", "DCA reduce · MT-SOL-0319"),
      id: "preview-fill-sol-1",
      pnl: "+7.31 USDT",
      symbol: "SOLUSDT",
    },
    {
      detail: pickText(lang, "0.004 BTC @ 88,100.00", "0.004 BTC @ 88,100.00"),
      event: pickText(lang, "网格卖出 · GB-BTC-1201", "Grid sell · GB-BTC-1201"),
      id: "preview-fill-btc-2",
      pnl: "+9.26 USDT",
      symbol: "BTCUSDT",
    },
    {
      detail: pickText(lang, "0.11 ETH @ 3,420.70", "0.11 ETH @ 3,420.70"),
      event: pickText(lang, "网格买入 · GB-ETH-0601", "Grid buy · GB-ETH-0601"),
      id: "preview-fill-eth-2",
      pnl: "-",
      symbol: "ETHUSDT",
    },
    {
      detail: pickText(lang, "0.6 BNB @ 651.40", "0.6 BNB @ 651.40"),
      event: pickText(lang, "网格买入 · GB-BNB-0087", "Grid buy · GB-BNB-0087"),
      id: "preview-fill-bnb-1",
      pnl: "-",
      symbol: "BNBUSDT",
    },
  ];
}

function previewExchangeTrades(): ExchangeTradeRow[] {
  return [
    {
      exchange: "Binance",
      fee_amount: "0.0436",
      fee_asset: "USDT",
      price: "87240.00",
      quantity: "0.005",
      side: "Sell",
      symbol: "BTCUSDT",
      trade_id: "binance-72818421",
      traded_at: "2026-06-15T09:42:18Z",
    },
    {
      exchange: "Binance Futures",
      fee_amount: "0.0189",
      fee_asset: "USDT",
      price: "3388.20",
      quantity: "0.22",
      side: "Buy",
      symbol: "ETHUSDT",
      trade_id: "binance-72817304",
      traded_at: "2026-06-15T09:18:02Z",
    },
    {
      exchange: "Binance",
      fee_amount: "0.0062",
      fee_asset: "BNB",
      price: "148.60",
      quantity: "8.4",
      side: "Sell",
      symbol: "SOLUSDT",
      trade_id: "binance-72816017",
      traded_at: "2026-06-15T08:56:44Z",
    },
    {
      exchange: "Binance",
      fee_amount: "0.0220",
      fee_asset: "USDT",
      price: "88100.00",
      quantity: "0.004",
      side: "Sell",
      symbol: "BTCUSDT",
      trade_id: "binance-72815072",
      traded_at: "2026-06-15T08:31:11Z",
    },
    {
      exchange: "Binance Futures",
      fee_amount: "0.0091",
      fee_asset: "USDT",
      price: "3420.70",
      quantity: "0.11",
      side: "Buy",
      symbol: "ETHUSDT",
      trade_id: "binance-72813904",
      traded_at: "2026-06-15T07:55:34Z",
    },
    {
      exchange: "Binance",
      fee_amount: "0.0044",
      fee_asset: "BNB",
      price: "651.40",
      quantity: "0.6",
      side: "Buy",
      symbol: "BNBUSDT",
      trade_id: "binance-72812006",
      traded_at: "2026-06-15T07:21:05Z",
    },
  ];
}

function previewAccountSnapshots(): AccountSnapshotRow[] {
  return [
    {
      captured_at: "2026-06-15T10:00:00Z",
      exchange: "Binance Spot",
      fees_paid: "2.84 USDT",
      funding_total: "0.00 USDT",
    },
    {
      captured_at: "2026-06-15T09:00:00Z",
      exchange: "Binance Futures",
      fees_paid: "1.16 USDT",
      funding_total: "-0.42 USDT",
    },
    {
      captured_at: "2026-06-14T23:00:00Z",
      exchange: "Binance Spot",
      fees_paid: "2.31 USDT",
      funding_total: "0.00 USDT",
    },
    {
      captured_at: "2026-06-14T19:00:00Z",
      exchange: "Binance Futures",
      fees_paid: "0.88 USDT",
      funding_total: "-0.18 USDT",
    },
    {
      captured_at: "2026-06-14T15:00:00Z",
      exchange: "Binance Spot",
      fees_paid: "1.92 USDT",
      funding_total: "0.00 USDT",
    },
    {
      captured_at: "2026-06-14T11:00:00Z",
      exchange: "Binance Futures",
      fees_paid: "0.73 USDT",
      funding_total: "+0.06 USDT",
    },
  ];
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
