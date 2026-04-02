import "server-only";

export type StrategyStatus = "draft" | "ready" | "running" | "paused" | "error_paused";
export type PreflightStatus = "idle" | "passed" | "failed";

export type StrategyRecord = {
  costBasis: string;
  fees: string;
  fillCount: number;
  fundingFees: string;
  generation: string;
  gridLevels: Array<{
    allocation: string;
    id: string;
    level: string;
    range: string;
    tp: string;
  }>;
  holdings: string;
  id: string;
  marketType: string;
  mode: string;
  name: string;
  netProfit: string;
  notional: string;
  orderCount: number;
  postTrigger: string;
  preflightChecks: Array<{
    id: string;
    item: string;
    result: "Pass" | "Fail";
  }>;
  preflightMessage: string | null;
  preflightStatus: PreflightStatus;
  realizedPnl: string;
  status: StrategyStatus;
  symbol: string;
  trailingTakeProfit: string;
  unrealizedPnl: string;
};

export type UserProductState = {
  billing: {
    currentPlan: string;
    graceEndsAt: string;
    membershipStatus: string;
    nextRenewalAt: string;
    orders: Array<{
      amount: string;
      chain: string;
      id: string;
      order: string;
      state: string;
      token: string;
    }>;
  };
  exchange: {
    apiKeyMasked: string | null;
    connectionMessage: string | null;
    connectionStatus: "idle" | "passed" | "failed";
    positionMode: string;
    saved: boolean;
    supportedScopes: string[];
    warning: string;
  };
  flash: {
    billing: string | null;
    exchange: string | null;
    security: string | null;
    strategy: string | null;
    telegram: string | null;
  };
  security: {
    passwordChangedAt: string | null;
    sessionsRevokedAt: string | null;
    totpEnabled: boolean;
  };
  telegram: {
    bindCode: string | null;
    bindCodeIssuedAt: string | null;
    boundAt: string | null;
    state: "unbound" | "code_issued" | "bound";
  };
  tradeHistory: Array<{
    activity: string;
    at: string;
    detail: string;
    id: string;
  }>;
  recentFills: Array<{
    id: string;
    pnl: string;
    side: string;
    state: string;
    symbol: string;
  }>;
  strategies: StrategyRecord[];
};

type Store = Map<string, UserProductState>;

const globalStore = globalThis as typeof globalThis & {
  __gridBinanceUserProductState?: Store;
};

const store = globalStore.__gridBinanceUserProductState ?? new Map<string, UserProductState>();
globalStore.__gridBinanceUserProductState = store;

export function getUserProductState(sessionToken: string | null | undefined): UserProductState {
  const key = sessionKey(sessionToken);

  if (!store.has(key)) {
    store.set(key, createDefaultState());
  }

  return structuredClone(store.get(key)!);
}


export async function getCurrentUserProductState() {
  const { cookies } = await import("next/headers");
  const cookieStore = await cookies();
  return getUserProductState(cookieStore.get("session_token")?.value ?? null);
}

export function updateUserProductState(
  sessionToken: string | null | undefined,
  updater: (state: UserProductState) => void,
): UserProductState {
  const next = getUserProductState(sessionToken);
  updater(next);
  store.set(sessionKey(sessionToken), next);
  return structuredClone(next);
}

export function slugifyStrategyName(value: string) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "strategy";
}

export function buildMaskedApiKey(value: string) {
  if (value.length <= 8) {
    return "••••";
  }

  return `${value.slice(0, 4)}••••${value.slice(-4)}`;
}

export function createStrategyRecord(input: {
  generation: string;
  marketType: string;
  mode: string;
  name: string;
  postTrigger?: string;
  symbol: string;
  trailingTakeProfit: string;
}): StrategyRecord {
  const id = slugifyStrategyName(input.name);

  return {
    costBasis: input.marketType === "spot" ? "4,820 USDT" : "2,100 USDT",
    fees: "-42.18 USDT",
    fillCount: 28,
    fundingFees: input.marketType === "spot" ? "0.00 USDT" : "-8.14 USDT",
    generation: input.generation,
    gridLevels: [
      { id: `${id}-l1`, level: "L1", range: "86,000 - 86,750", allocation: "0.008 BTC", tp: "1.2%" },
      { id: `${id}-l2`, level: "L2", range: "86,750 - 87,500", allocation: "0.007 BTC", tp: "1.1%" },
      { id: `${id}-l3`, level: "L3", range: "87,500 - 88,250", allocation: "0.006 BTC", tp: "0.9%" },
    ],
    holdings: input.marketType === "spot" ? "0.021 BTC" : "0.38 contract equivalent",
    id,
    marketType: input.marketType,
    mode: input.mode,
    name: input.name,
    netProfit: "+164.24 USDT",
    notional: input.marketType === "spot" ? "5,000 USDT" : "2,400 USDT",
    orderCount: 34,
    postTrigger: input.postTrigger ?? "rebuild",
    preflightChecks: [
      { id: `${id}-check-1`, item: "Exchange filters", result: "Pass" },
      { id: `${id}-check-2`, item: "Balance coverage", result: "Pass" },
      { id: `${id}-check-3`, item: "Hedge mode", result: input.marketType === "spot" ? "Pass" : "Fail" },
    ],
    preflightMessage: null,
    preflightStatus: "idle",
    realizedPnl: "+192.44 USDT",
    status: "draft",
    symbol: input.symbol,
    trailingTakeProfit: input.trailingTakeProfit,
    unrealizedPnl: "-28.20 USDT",
  };
}

export function findStrategy(state: UserProductState, id: string) {
  return state.strategies.find((strategy) => strategy.id === id) ?? null;
}

function createDefaultState(): UserProductState {
  const btc = createStrategyRecord({
    generation: "geometric",
    marketType: "spot",
    mode: "classic",
    name: "BTC Recovery Ladder",
    symbol: "BTCUSDT",
    trailingTakeProfit: "0.8",
  });
  btc.status = "running";
  btc.preflightStatus = "passed";
  btc.preflightMessage = "Exchange filters, balance, and hedge-mode checks passed.";

  const eth = createStrategyRecord({
    generation: "arithmetic",
    marketType: "usd-m",
    mode: "short",
    name: "ETH Short Ladder",
    symbol: "ETHUSDT",
    trailingTakeProfit: "0.5",
  });
  eth.status = "error_paused";
  eth.preflightStatus = "failed";
  eth.preflightMessage = "Hedge mode must be enabled before futures strategy restart.";
  eth.preflightChecks = [
    { id: "eth-check-1", item: "Exchange filters", result: "Pass" },
    { id: "eth-check-2", item: "Balance coverage", result: "Pass" },
    { id: "eth-check-3", item: "Hedge mode", result: "Fail" },
  ];

  return {
    billing: {
      currentPlan: "Monthly",
      graceEndsAt: "2026-04-17",
      membershipStatus: "Active",
      nextRenewalAt: "2026-04-15",
      orders: [
        { id: "order-4138", order: "ORD-4138", chain: "Solana", token: "USDC", amount: "60.00", state: "Confirmed" },
      ],
    },
    exchange: {
      apiKeyMasked: null,
      connectionMessage: null,
      connectionStatus: "idle",
      positionMode: "hedge",
      saved: false,
      supportedScopes: ["Spot", "USDⓈ-M", "COIN-M"],
      warning: "Do not enable withdrawal permission on your Binance API key.",
    },
    flash: {
      billing: null,
      exchange: null,
      security: null,
      strategy: null,
      telegram: null,
    },
    recentFills: [
      { id: "fill-1", symbol: "BTCUSDT", side: "Buy", pnl: "+82.10", state: "Settled" },
      { id: "fill-2", symbol: "ETHUSDT", side: "Sell", pnl: "+24.87", state: "Settled" },
      { id: "fill-3", symbol: "SOLUSDT", side: "Buy", pnl: "-6.24", state: "Trailing TP" },
    ],
    security: {
      passwordChangedAt: null,
      sessionsRevokedAt: null,
      totpEnabled: false,
    },
    strategies: [btc, eth],
    telegram: {
      bindCode: null,
      bindCodeIssuedAt: null,
      boundAt: null,
      state: "unbound",
    },
    tradeHistory: [
      { id: "hist-1", at: "2026-04-02 09:21", activity: "API credential retest", detail: "Pending" },
      { id: "hist-2", at: "2026-04-02 08:55", activity: "Billing order created", detail: "Awaiting exact transfer" },
      { id: "hist-3", at: "2026-04-01 23:14", activity: "Strategy auto-pause", detail: "Runtime anomaly surfaced" },
    ],
  };
}

function sessionKey(sessionToken: string | null | undefined) {
  return sessionToken?.trim() || "anonymous";
}
