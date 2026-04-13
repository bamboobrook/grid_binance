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
    payload = buildStrategyPayload(formData);
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

function buildStrategyPayload(formData: FormData) {
  const market = mapMarket(readField(formData, "marketType") || "spot");
  const generation = mapGeneration(readField(formData, "generation") || "custom");
  const strategyType = readField(formData, "strategyType") || "ordinary_grid";
  const symbol = readField(formData, "symbol");
  if (!symbol) {
    throw new Error("Symbol must be selected from the search results.");
  }

  validateClassicGridCount(formData, strategyType);

  return {
    name: readField(formData, "name") || "Strategy Draft",
    symbol,
    market,
    mode: mapMode(readField(formData, "mode") || "classic"),
    strategy_type: strategyType,
    generation,
    amount_mode: mapAmountMode(readField(formData, "amountMode") || "quote"),
    futures_margin_mode: market === "Spot" ? null : mapFuturesMarginMode(readField(formData, "futuresMarginMode") || "isolated"),
    leverage: market === "Spot" ? null : readPositiveInteger(formData, "leverage", "Leverage"),
    levels: parseLevelsJson(readField(formData, "levels_json"), strategyType),
    overall_take_profit_bps: readPercentField(formData, "overallTakeProfit", true),
    overall_stop_loss_bps: readPercentField(formData, "overallStopLoss", false),
    reference_price_source: mapReferencePriceSource(readField(formData, "referencePriceMode") || "manual"),
    post_trigger_action: mapPostTrigger(readField(formData, "postTrigger") || "rebuild"),
  };
}

function validateClassicGridCount(formData: FormData, strategyType: string) {
  if (strategyType !== "classic_bilateral_grid") {
    return;
  }

  const gridCount = Number.parseInt(readField(formData, "gridCount"), 10);
  if (!Number.isFinite(gridCount) || gridCount < 2) {
    throw new Error("Classic bilateral grid requires at least 2 levels.");
  }
}

function parseLevelsJson(raw: string, strategyType: string): ParsedGridLevel[] {
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

  const levels = parsed.map((level, index) => parseLevel(level, index));
  if (strategyType === "classic_bilateral_grid" && levels.length < 2) {
    throw new Error("Classic bilateral grid requires at least 2 levels.");
  }

  return levels;
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

function readPositiveInteger(formData: FormData, key: string, label: string) {
  const parsed = Number.parseInt(readField(formData, key), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
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

function mapReferencePriceSource(value: string) {
  return value === "market" ? "market" : "manual";
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
