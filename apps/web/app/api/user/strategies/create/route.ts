import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type ParsedGridLevel = {
  entry_price: string;
  quantity: string;
  take_profit_bps: number;
  trailing_bps: number | null;
};

export async function POST(request: Request) {
  const formData = await request.formData();
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return redirectToPublic(request, "/login?error=session+expired");
  }

  let payload;
  try {
    payload = await buildStrategyPayload(formData);
  } catch (error) {
    return redirectToApp(request, `/strategies/new?error=${encodeURIComponent(readErrorMessage(error))}`);
  }

  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(payload),
    cache: "no-store",
  });

  if (!response.ok) {
    return redirectToApp(request, `/strategies/new?error=${encodeURIComponent(await readError(response))}`);
  }

  const created = (await response.json()) as { id: string };
  return redirectToApp(request, `/strategies/${created.id}?notice=draft-saved`);
}

async function buildStrategyPayload(formData: FormData) {
  const market = mapMarket(readField(formData, "marketType") || "spot");
  const generation = mapGeneration(readField(formData, "generation") || "custom");

  const symbol = readField(formData, "symbol");
  if (!symbol) {
    throw new Error("Symbol must be selected from the search results.");
  }

  return {
    name: readField(formData, "name") || "Strategy Draft",
    symbol,
    market,
    mode: mapMode(readField(formData, "mode") || "classic"),
    generation,
    amount_mode: mapAmountMode(readField(formData, "amountMode") || "quote"),
    futures_margin_mode: market === "Spot" ? null : mapFuturesMarginMode(readField(formData, "futuresMarginMode") || "isolated"),
    leverage: market === "Spot" ? null : readPositiveInteger(formData, "leverage", "Leverage"),
    levels: await readLevels(formData, generation, market, symbol),
    overall_take_profit_bps: readPercentField(formData, "overallTakeProfit", true),
    overall_stop_loss_bps: readPercentField(formData, "overallStopLoss", false),
    post_trigger_action: mapPostTrigger(readField(formData, "postTrigger") || "rebuild"),
  };
}

async function readLevels(formData: FormData, generation: "Arithmetic" | "Geometric" | "Custom", market: "Spot" | "FuturesUsdM" | "FuturesCoinM", symbol: string) {
  const editorMode = readField(formData, "editorMode") || "custom";
  if (editorMode === "batch" && generation !== "Custom") {
    return buildBatchLevels(formData, generation, market, symbol);
  }
  return parseLevelsJson(readField(formData, "levels_json"));
}

async function buildBatchLevels(formData: FormData, generation: "Arithmetic" | "Geometric", market: "Spot" | "FuturesUsdM" | "FuturesCoinM", symbol: string) {
  const referencePrice = await resolveReferencePrice(formData, market, symbol);
  const gridCount = readPositiveInteger(formData, "gridCount", "Grid count");
  const gridSpacingPercent = readPositiveNumber(formData, "gridSpacingPercent", "Batch spacing (%)");
  const takeProfitPercent = readPositiveNumber(formData, "batchTakeProfit", "Batch take profit (%)");
  const trailingPercent = readOptionalPositiveNumber(formData, "batchTrailing", "Batch trailing take profit (%)");
  const amountMode = readField(formData, "amountMode") || "quote";
  const baseQuantity = amountMode === "base"
    ? readPositiveNumber(formData, "baseQuantity", "Base asset quantity")
    : null;
  const quoteAmount = amountMode === "quote"
    ? readPositiveNumber(formData, "quoteAmount", "Quote amount (USDT)")
    : null;
  const takeProfitBps = Math.round(takeProfitPercent * 100);
  const trailingBps = trailingPercent === null ? null : Math.round(trailingPercent * 100);

  if (trailingBps !== null && trailingBps > takeProfitBps) {
    throw new Error("Batch trailing take profit (%) cannot exceed batch take profit (%).");
  }

  const midpoint = (gridCount - 1) / 2;
  return Array.from({ length: gridCount }, (_value, index) => {
    const offset = index - midpoint;
    const spacingFactor = gridSpacingPercent / 100;
    const rawPrice = generation === "Geometric"
      ? referencePrice * Math.pow(1 + spacingFactor, offset)
      : referencePrice * (1 + spacingFactor * offset);
    if (!Number.isFinite(rawPrice) || rawPrice <= 0) {
      throw new Error("Generated grid price must stay above 0. Reduce spacing or grid count.");
    }
    const quantity = amountMode === "quote"
      ? (quoteAmount ?? 0) / rawPrice
      : (baseQuantity ?? 0);
    if (!Number.isFinite(quantity) || quantity <= 0) {
      throw new Error("Generated grid quantity must stay above 0.");
    }
    return {
      entry_price: formatDecimal(rawPrice, 8),
      quantity: formatDecimal(quantity, 8),
      take_profit_bps: takeProfitBps,
      trailing_bps: trailingBps,
    };
  });
}

function parseLevelsJson(raw: string): ParsedGridLevel[] {
  if (!raw) {
    throw new Error("Grid levels JSON is required.");
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("Grid levels JSON must be valid JSON.");
  }

  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error("Grid levels JSON must be a non-empty array.");
  }

  return parsed.map((level, index) => parseLevel(level, index));
}

function parseLevel(level: unknown, index: number): ParsedGridLevel {
  if (!level || typeof level !== "object") {
    throw new Error(`Grid level ${index + 1} must be an object.`);
  }

  const entryPrice = readJsonString(level, "entry_price");
  const quantity = readJsonString(level, "quantity");
  const takeProfitBps = readJsonInteger(level, "take_profit_bps");
  const trailingBps = readJsonOptionalInteger(level, "trailing_bps");

  if (!entryPrice || !quantity) {
    throw new Error(`Grid level ${index + 1} must include entry_price and quantity.`);
  }
  if (takeProfitBps <= 0) {
    throw new Error(`Grid level ${index + 1} take_profit_bps must be greater than 0.`);
  }
  if (trailingBps !== null && trailingBps > takeProfitBps) {
    throw new Error(`Grid level ${index + 1} trailing_bps cannot exceed take_profit_bps.`);
  }

  return {
    entry_price: entryPrice,
    quantity,
    take_profit_bps: takeProfitBps,
    trailing_bps: trailingBps,
  };
}

function readJsonString(value: object, key: string) {
  const candidate = Reflect.get(value, key);
  if (typeof candidate === "number") {
    return String(candidate);
  }
  return typeof candidate === "string" ? candidate.trim() : "";
}

function readJsonInteger(value: object, key: string) {
  const candidate = Reflect.get(value, key);
  const parsed = typeof candidate === "number" ? candidate : Number.parseInt(String(candidate ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : 0;
}

function readJsonOptionalInteger(value: object, key: string) {
  const candidate = Reflect.get(value, key);
  if (candidate === null || candidate === undefined || candidate === "") {
    return null;
  }
  const parsed = typeof candidate === "number" ? candidate : Number.parseInt(String(candidate), 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function readPercentField(formData: FormData, key: string, required: boolean) {
  const value = readField(formData, key);
  if (!value) {
    if (required) {
      throw new Error("Overall take profit (%) is required.");
    }
    return null;
  }

  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${key === "overallTakeProfit" ? "Overall take profit" : "Overall stop loss"} (%) must be a positive number.`);
  }
  return Math.round(parsed * 100);
}

function readPositiveNumber(formData: FormData, key: string, label: string) {
  const parsed = Number.parseFloat(readField(formData, key));
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive number.`);
  }
  return parsed;
}

function readOptionalPositiveNumber(formData: FormData, key: string, label: string) {
  const value = readField(formData, key);
  if (!value) {
    return null;
  }
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive number when provided.`);
  }
  return parsed;
}

function readPositiveInteger(formData: FormData, key: string, label: string) {
  const parsed = Number.parseInt(readField(formData, key), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

function formatDecimal(value: number, scale: number) {
  const normalized = value.toFixed(scale).replace(/\.0+$/, "").replace(/(\.\d*?)0+$/, "$1");
  return normalized === "-0" ? "0" : normalized;
}

async function resolveReferencePrice(
  formData: FormData,
  market: "Spot" | "FuturesUsdM" | "FuturesCoinM",
  symbol: string,
) {
  const mode = readField(formData, "referencePriceMode") || "manual";
  if (mode !== "market") {
    return readPositiveNumber(formData, "referencePrice", "Reference price");
  }
  const response = await fetch(binanceTickerUrl(market, symbol), { cache: "no-store" });
  if (!response.ok) {
    throw new Error("Current market price is temporarily unavailable.");
  }
  const payload = (await response.json()) as { price?: string };
  const parsed = Number.parseFloat(String(payload.price ?? ""));
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error("Current market price is temporarily unavailable.");
  }
  return parsed;
}

function binanceTickerUrl(
  market: "Spot" | "FuturesUsdM" | "FuturesCoinM",
  symbol: string,
) {
  const encodedSymbol = encodeURIComponent(symbol.trim().toUpperCase());
  switch (market) {
    case "FuturesUsdM":
      return `https://fapi.binance.com/fapi/v1/ticker/price?symbol=${encodedSymbol}`;
    case "FuturesCoinM":
      return `https://dapi.binance.com/dapi/v1/ticker/price?symbol=${encodedSymbol}`;
    default:
      return `https://api.binance.com/api/v3/ticker/price?symbol=${encodedSymbol}`;
  }
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function mapAmountMode(value: string) {
  return value === "base" ? "Base" : "Quote";
}

function mapFuturesMarginMode(value: string) {
  return value === "cross" ? "Cross" : "Isolated";
}

function mapMarket(value: string) {
  switch (value) {
    case "usd-m":
      return "FuturesUsdM";
    case "coin-m":
      return "FuturesCoinM";
    default:
      return "Spot";
  }
}

function mapMode(value: string) {
  switch (value) {
    case "buy-only":
      return "SpotBuyOnly";
    case "sell-only":
      return "SpotSellOnly";
    case "long":
      return "FuturesLong";
    case "short":
      return "FuturesShort";
    case "neutral":
      return "FuturesNeutral";
    default:
      return "SpotClassic";
  }
}

function mapGeneration(value: string) {
  switch (value) {
    case "arithmetic":
      return "Arithmetic" as const;
    case "geometric":
      return "Geometric" as const;
    default:
      return "Custom" as const;
  }
}

function mapPostTrigger(value: string) {
  return value === "stop" ? "Stop" : "Rebuild";
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "strategy request failed";
  } catch {
    return "strategy request failed";
  }
}

function readErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "strategy request failed";
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function redirectToApp(request: Request, pathname: string) {
  return NextResponse.redirect(publicUrl(request, localizedAppPath(request, pathname)), { status: 303 });
}

function redirectToPublic(request: Request, pathname: string) {
  return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, pathname)), { status: 303 });
}
