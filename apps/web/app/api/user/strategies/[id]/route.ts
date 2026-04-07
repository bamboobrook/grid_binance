import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type BackendStrategy = {
  id: string;
  name: string;
  symbol: string;
  market: "Spot" | "FuturesUsdM" | "FuturesCoinM";
  mode: "SpotClassic" | "SpotBuyOnly" | "SpotSellOnly" | "FuturesLong" | "FuturesShort" | "FuturesNeutral";
  draft_revision: {
    generation: "Arithmetic" | "Geometric" | "Custom";
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
    post_trigger_action: "Stop" | "Rebuild";
  };
  status: string;
};

type ParsedGridLevel = {
  entry_price: string;
  quantity: string;
  take_profit_bps: number;
  trailing_bps: number | null;
};

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  const current = await fetchStrategy(sessionToken, id);
  if (!current) {
    return NextResponse.redirect(new URL(`/app/strategies/${id}?error=strategy+workspace+is+temporarily+unavailable`, request.url), { status: 303 });
  }

  if (intent === "pause") {
    const paused = await strategyPost(sessionToken, "/strategies/batch/pause", { ids: [id] });
    if (!paused.ok) {
      return redirectWithError(request, id, await readError(paused.response));
    }
    const pausedPayload = (await paused.response.json()) as { paused?: number };
    if ((pausedPayload.paused ?? 0) === 0) {
      return redirectWithError(request, id, "No running strategy was paused.");
    }
    return NextResponse.redirect(new URL(`/app/strategies/${id}?notice=strategy-paused`, request.url), { status: 303 });
  }

  if (intent === "stop") {
    const stopped = await strategyPost(sessionToken, `/strategies/${id}/stop`, null);
    if (!stopped.ok) {
      return redirectWithError(request, id, await readError(stopped.response));
    }
    return NextResponse.redirect(new URL(`/app/strategies/${id}?notice=strategy-stopped`, request.url), { status: 303 });
  }

  if (intent === "delete") {
    const deleted = await strategyPost(sessionToken, "/strategies/batch/delete", { ids: [id] });
    if (!deleted.ok) {
      return redirectWithError(request, id, await readError(deleted.response));
    }
    const deletedPayload = (await deleted.response.json()) as { deleted?: number };
    if ((deletedPayload.deleted ?? 0) === 0) {
      return redirectWithError(request, id, "Strategy cannot be deleted while orders or positions remain.");
    }
    return NextResponse.redirect(new URL(`/app/strategies?notice=strategy-deleted`, request.url), { status: 303 });
  }

  if (intent === "save") {
    if (current.status === "Running") {
      return redirectWithError(request, id, "Strategy must be paused before editing and saving changes.");
    }
    let payload;
    try {
      payload = buildUpdatePayload(formData, current);
    } catch (error) {
      return redirectWithError(request, id, readErrorMessage(error));
    }
    const saved = await strategyPut(sessionToken, `/strategies/${id}`, payload);
    if (!saved.ok) {
      return redirectWithError(request, id, await readError(saved.response));
    }
    return NextResponse.redirect(new URL(`/app/strategies/${id}?notice=edits-saved`, request.url), { status: 303 });
  }

  if (intent === "preflight") {
    const preflight = await strategyPost(sessionToken, `/strategies/${id}/preflight`, null);
    if (!preflight.ok) {
      return redirectWithError(request, id, await readError(preflight.response));
    }
    const payload = (await preflight.response.json()) as { ok: boolean; failures?: Array<{ guidance?: string; reason?: string; step: string }> };
    if (payload.ok) {
      return NextResponse.redirect(new URL(`/app/strategies/${id}?notice=preflight-passed`, request.url), { status: 303 });
    }
    const failure = payload.failures?.[0];
    const url = new URL(`/app/strategies/${id}`, request.url);
    url.searchParams.set("notice", "preflight-failed");
    if (failure?.step) url.searchParams.set("step", failure.step);
    if (failure) url.searchParams.set("reason", humanizeFailure(failure.step, failure.guidance ?? failure.reason ?? ""));
    return NextResponse.redirect(url, { status: 303 });
  }

  const path = current.status === "Paused" ? `/strategies/${id}/resume` : `/strategies/${id}/start`;
  const started = await strategyPost(sessionToken, path, null);
  if (!started.ok) {
    const parsed = await readStrategyError(started.response);
    const url = new URL(`/app/strategies/${id}`, request.url);
    url.searchParams.set("notice", "start-failed");
    url.searchParams.set("error", parsed.error);
    if (parsed.reason) {
      url.searchParams.set("reason", parsed.reason);
    }
    return NextResponse.redirect(url, { status: 303 });
  }
  return NextResponse.redirect(new URL(`/app/strategies/${id}?notice=strategy-started`, request.url), { status: 303 });
}

function buildUpdatePayload(formData: FormData, current: BackendStrategy) {
  const generation = (mapGeneration(readField(formData, "generation")) || current.draft_revision.generation);
  const market = mapMarket(readField(formData, "marketType")) || current.market;
  return {
    name: readField(formData, "name") || current.name,
    symbol: readField(formData, "symbol") || current.symbol,
    market,
    mode: mapMode(readField(formData, "mode")) || current.mode,
    generation,
    amount_mode: mapAmountMode(readField(formData, "amountMode")) || current.draft_revision.amount_mode || "Quote",
    futures_margin_mode: market === "Spot"
      ? null
      : mapFuturesMarginMode(readField(formData, "futuresMarginMode")) || current.draft_revision.futures_margin_mode || "Isolated",
    leverage: market === "Spot"
      ? null
      : readPositiveInteger(formData, "leverage", "Leverage"),
    levels: readLevels(formData, generation, current.draft_revision.levels),
    overall_take_profit_bps: readPercentField(formData, "overallTakeProfit", current.draft_revision.overall_take_profit_bps),
    overall_stop_loss_bps: readPercentField(formData, "overallStopLoss", current.draft_revision.overall_stop_loss_bps),
    post_trigger_action: mapPostTrigger(readField(formData, "postTrigger")) || current.draft_revision.post_trigger_action,
  };
}

function readLevels(
  formData: FormData,
  generation: BackendStrategy["draft_revision"]["generation"],
  fallback: BackendStrategy["draft_revision"]["levels"],
): ParsedGridLevel[] {
  const editorMode = readField(formData, "editorMode") || "custom";
  if (editorMode === "batch" && generation !== "Custom") {
    return buildBatchLevels(formData, generation);
  }
  return parseLevelsJson(readField(formData, "levels_json"), fallback);
}

function buildBatchLevels(formData: FormData, generation: "Arithmetic" | "Geometric") {
  const referencePrice = readPositiveNumber(formData, "referencePrice", "Reference price");
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

function parseLevelsJson(raw: string, fallback: BackendStrategy["draft_revision"]["levels"]): ParsedGridLevel[] {
  if (!raw) {
    return fallback;
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

function readPercentField(formData: FormData, key: string, fallback: number | null) {
  const value = readField(formData, key);
  if (!value) {
    return fallback;
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

async function fetchStrategy(sessionToken: string, strategyId: string) {
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  const payload = (await response.json()) as { items: BackendStrategy[] };
  return payload.items.find((item) => item.id === strategyId) ?? null;
}

async function strategyPost(sessionToken: string, path: string, body: unknown) {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      ...(body === null ? {} : { "content-type": "application/json" }),
    },
    body: body === null ? undefined : JSON.stringify(body),
    cache: "no-store",
  });
  return { ok: response.ok, response };
}

async function strategyPut(sessionToken: string, path: string, body: unknown) {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "PUT",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });
  return { ok: response.ok, response };
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function mapAmountMode(value: string) {
  if (value === "Base") return "Base";
  if (value === "Quote") return "Quote";
  return value === "base" ? "Base" : value === "quote" ? "Quote" : null;
}

function mapFuturesMarginMode(value: string) {
  if (value === "Cross") return "Cross";
  if (value === "Isolated") return "Isolated";
  return value === "cross" ? "Cross" : value === "isolated" ? "Isolated" : null;
}

function mapMarket(value: string) {
  switch (value) {
    case "usd-m":
      return "FuturesUsdM";
    case "coin-m":
      return "FuturesCoinM";
    default:
      return null;
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
    case "classic":
      return "SpotClassic";
    default:
      return null;
  }
}

function mapGeneration(value: string) {
  switch (value) {
    case "arithmetic":
      return "Arithmetic" as const;
    case "geometric":
      return "Geometric" as const;
    case "custom":
      return "Custom" as const;
    default:
      return null;
  }
}

function mapPostTrigger(value: string) {
  if (value === "stop") {
    return "Stop" as const;
  }
  if (value === "rebuild") {
    return "Rebuild" as const;
  }
  return null;
}

function redirectWithError(request: Request, strategyId: string, error: string) {
  return NextResponse.redirect(new URL(`/app/strategies/${strategyId}?error=${encodeURIComponent(error)}`, request.url), { status: 303 });
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

async function readStrategyError(response: Response) {
  try {
    const payload = (await response.json()) as {
      error?: string;
      preflight?: { failures?: Array<{ guidance?: string | null; reason?: string | null; step: string }> };
    };
    const failure = payload.preflight?.failures?.[0];
    return {
      error: payload.error ?? "strategy request failed",
      reason: failure ? humanizeFailure(failure.step, failure.guidance ?? failure.reason ?? "") : null,
    };
  } catch {
    return { error: "strategy request failed", reason: null };
  }
}

function humanizeFailure(step: string, message: string) {
  const detail = message.trim();
  if (step === "membership") {
    return detail || "Renew or reactivate membership before starting this strategy.";
  }
  return detail || `Resolve the ${step} pre-flight check before retrying.`;
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
