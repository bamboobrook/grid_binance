import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type BackendStrategy = {
  id: string;
  name: string;
  symbol: string;
  market: "Spot" | "FuturesUsdM" | "FuturesCoinM";
  mode: "SpotClassic" | "SpotBuyOnly" | "SpotSellOnly" | "FuturesLong" | "FuturesShort" | "FuturesNeutral";
  strategy_type?: "ordinary_grid" | "classic_bilateral_grid";
  tags: string[];
  notes: string;
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
    reference_price_source?: "manual" | "market";
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
  const returnTo = readField(formData, "returnTo");
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return redirectToPublic(request, "/login?error=session+expired");
  }

  const current = await fetchStrategy(sessionToken, id);
  if (!current) {
    return redirectWithError(request, id, "strategy workspace is temporarily unavailable", returnTo);
  }

  if (intent === "pause") {
    if (current.status !== "Running") {
      return redirectWithError(request, id, pauseErrorForStatus(current.status), returnTo);
    }
    const paused = await strategyPost(sessionToken, "/strategies/batch/pause", { ids: [id] });
    if (!paused.ok) {
      return redirectWithError(request, id, await readError(paused.response), returnTo);
    }
    const pausedPayload = (await paused.response.json()) as { paused?: number; failures?: Array<{ error?: string }> };
    if ((pausedPayload.paused ?? 0) === 0) {
      return redirectWithError(request, id, pausedPayload.failures?.[0]?.error ?? "Strategy action failed.", returnTo);
    }
    return redirectWithNotice(request, id, "strategy-paused", returnTo);
  }

  if (intent === "stop") {
    if (current.status !== "Running" && current.status !== "Paused" && current.status !== "ErrorPaused") {
      return redirectWithError(request, id, stopErrorForStatus(current.status), returnTo);
    }
    const stopped = await strategyPost(sessionToken, `/strategies/${id}/stop`, null);
    if (!stopped.ok) {
      return redirectWithError(request, id, await readError(stopped.response), returnTo);
    }
    return redirectWithNotice(request, id, "strategy-stopped", returnTo);
  }

  if (intent === "delete") {
    if (current.status === "Running") {
      return redirectWithError(request, id, deleteErrorForStatus(current.status), returnTo);
    }
    const deleted = await strategyPost(sessionToken, "/strategies/batch/delete", { ids: [id] });
    if (!deleted.ok) {
      return redirectWithError(request, id, await readError(deleted.response), returnTo);
    }
    const deletedPayload = (await deleted.response.json()) as { deleted?: number; failures?: Array<{ error?: string }> };
    if ((deletedPayload.deleted ?? 0) === 0) {
      return redirectWithError(request, id, deletedPayload.failures?.[0]?.error ?? "Strategy action failed.", returnTo);
    }
    return redirectWithNotice(request, id, "strategy-deleted", returnTo);
  }

  if (intent === "save") {
    if (current.status === "Running") {
      return redirectWithError(request, id, "Strategy must be paused before editing and saving changes.", returnTo);
    }
    let payload;
    try {
      payload = await buildUpdatePayload(formData, current);
    } catch (error) {
      return redirectWithError(request, id, readErrorMessage(error), returnTo);
    }
    const saved = await strategyPut(sessionToken, `/strategies/${id}`, payload);
    if (!saved.ok) {
      return redirectWithError(request, id, await readError(saved.response), returnTo);
    }
    return redirectWithNotice(request, id, "edits-saved", returnTo);
  }

  if (intent === "preflight") {
    const preflight = await strategyPost(sessionToken, `/strategies/${id}/preflight`, null);
    if (!preflight.ok) {
      return redirectWithError(request, id, await readError(preflight.response), returnTo);
    }
    const payload = (await preflight.response.json()) as { ok: boolean; failures?: Array<{ guidance?: string; reason?: string; step: string }> };
    if (payload.ok) {
      return redirectWithNotice(request, id, "preflight-passed", returnTo);
    }
    const failure = payload.failures?.[0];
    const url = preferredStrategyUrl(request, id, returnTo);
    url.searchParams.set("notice", "preflight-failed");
    if (returnTo !== "list") {
      if (failure?.step) url.searchParams.set("step", failure.step);
      if (failure) url.searchParams.set("reason", humanizeFailure(failure.step, failure.guidance ?? failure.reason ?? ""));
    }
    return NextResponse.redirect(url, { status: 303 });
  }

  const path = current.status === "Paused" ? `/strategies/${id}/resume` : `/strategies/${id}/start`;
  const started = await strategyPost(sessionToken, path, null);
  if (!started.ok) {
    const parsed = await readStrategyError(started.response);
    const url = preferredStrategyUrl(request, id, returnTo);
    url.searchParams.set("notice", "start-failed");
    url.searchParams.set("error", parsed.error);
    if (parsed.reason && returnTo !== "list") {
      url.searchParams.set("reason", parsed.reason);
    }
    return NextResponse.redirect(url, { status: 303 });
  }
  return redirectWithNotice(request, id, "strategy-started", returnTo);
}

async function buildUpdatePayload(formData: FormData, current: BackendStrategy) {
  const generation = mapGeneration(readField(formData, "generation")) || current.draft_revision.generation;
  const market = mapMarket(readField(formData, "marketType")) || current.market;
  const symbol = readField(formData, "symbol") || current.symbol;
  const strategyType = mapStrategyType(readField(formData, "strategyType")) || current.strategy_type || "ordinary_grid";

  validateClassicGridCount(formData, strategyType);

  const referencePrice = readField(formData, "referencePrice");
  const tagsRaw = readField(formData, "tags");
  const tags = tagsRaw ? tagsRaw.split(",").map((t) => t.trim()).filter((t) => t.length > 0) : current.tags;
  const notes = formData.has("notes") ? readField(formData, "notes") : current.notes;

  return {
    name: formData.has("name") ? readField(formData, "name") : current.name,
    symbol,
    market,
    mode: mapMode(readField(formData, "mode")) || current.mode,
    strategy_type: strategyType,
    generation,
    amount_mode: mapAmountMode(readField(formData, "amountMode")) || current.draft_revision.amount_mode || "Quote",
    futures_margin_mode: market === "Spot"
      ? null
      : mapFuturesMarginMode(readField(formData, "futuresMarginMode")) || current.draft_revision.futures_margin_mode || "Isolated",
    leverage: market === "Spot"
      ? null
      : readPositiveInteger(formData, "leverage", "Leverage"),
    levels: parseLevelsJson(readField(formData, "levels_json"), current.draft_revision.levels, strategyType),
    overall_take_profit_bps: readPercentField(formData, "overallTakeProfit", current.draft_revision.overall_take_profit_bps),
    overall_stop_loss_bps: readPercentField(formData, "overallStopLoss", current.draft_revision.overall_stop_loss_bps),
    reference_price_source: mapReferencePriceSource(readField(formData, "referencePriceMode"))
      || current.draft_revision.reference_price_source
      || "manual",
    ...(referencePrice ? { reference_price: referencePrice } : {}),
    post_trigger_action: mapPostTrigger(readField(formData, "postTrigger")) || current.draft_revision.post_trigger_action,
    tags,
    notes,
  };
}

function validateClassicGridCount(formData: FormData, strategyType: BackendStrategy["strategy_type"] | string) {
  if (strategyType !== "classic_bilateral_grid") {
    return;
  }

  const gridCount = Number.parseInt(readField(formData, "gridCount"), 10);
  if (!Number.isFinite(gridCount) || gridCount < 2) {
    throw new Error("Classic bilateral grid requires at least 2 levels.");
  }
}

function parseLevelsJson(
  raw: string,
  fallback: BackendStrategy["draft_revision"]["levels"],
  strategyType: BackendStrategy["strategy_type"] | string,
): ParsedGridLevel[] {
  if (!raw) {
    if (strategyType === "classic_bilateral_grid" && fallback.length < 2) {
      throw new Error("Classic bilateral grid requires at least 2 levels.");
    }
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

function readPositiveInteger(formData: FormData, key: string, label: string) {
  const parsed = Number.parseInt(readField(formData, key), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

async function fetchStrategy(sessionToken: string, strategyId: string) {
  const response = await fetch(`${authApiBaseUrl()}/strategies/${strategyId}`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as BackendStrategy | null;
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

function mapStrategyType(value: string) {
  if (value === "ordinary_grid" || value === "classic_bilateral_grid") {
    return value;
  }
  return null;
}

function mapReferencePriceSource(value: string) {
  if (value === "manual" || value === "market") {
    return value;
  }
  return null;
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

function redirectWithError(request: Request, strategyId: string, error: string, returnTo = "") {
  const url = preferredStrategyUrl(request, strategyId, returnTo);
  url.searchParams.set("error", error);
  return NextResponse.redirect(url, { status: 303 });
}

function redirectWithNotice(request: Request, strategyId: string, notice: string, returnTo = "") {
  if (returnTo === "list") {
    if (notice === "strategy-started") {
      return redirectToApp(request, "/strategies?notice=strategy-started");
    }
    if (notice === "strategy-stopped") {
      return redirectToApp(request, "/strategies?notice=strategy-stopped");
    }
    return redirectToApp(request, `/strategies?notice=${notice}`);
  }
  return redirectToDetail(request, strategyId, `?notice=${notice}`);
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
  if (step === "membership" || step === "membership_status") {
    return detail || "请先续费或恢复会员资格后再启动该策略。 / Renew or reactivate membership before starting this strategy.";
  }
  return detail || `请先处理 ${step} 预检项后再重试。 / Resolve the ${step} pre-flight check before retrying.`;
}

function pauseErrorForStatus(status: string) {
  switch (status) {
    case "Draft":
    case "Stopped":
      return "Strategy has not started yet; only running strategies can pause.";
    case "Paused":
      return "Strategy is already paused.";
    case "ErrorPaused":
      return "Strategy is already blocked. Review the runtime error before retrying.";
    case "Archived":
      return "Strategy has already been deleted.";
    default:
      return "Strategy is not in a pausable state.";
  }
}

function stopErrorForStatus(status: string) {
  if (status === "Archived") {
    return "Strategy has already been deleted.";
  }
  return "Strategy has not started yet; only running, blocked, or paused strategies can stop.";
}

function deleteErrorForStatus(status: string) {
  switch (status) {
    case "Running":
      return "Pause or stop the running strategy before deleting it.";
    case "Archived":
      return "Strategy has already been deleted.";
    default:
      return "Strategy cannot be deleted while orders or positions remain.";
  }
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function preferredStrategyUrl(request: Request, strategyId: string, returnTo = "") {
  if (returnTo === "list") {
    return publicUrl(request, localizedAppPath(request, "/strategies"));
  }
  return publicUrl(request, localizedAppPath(request, `/strategies/${strategyId}`));
}

function redirectToApp(request: Request, pathname: string) {
  return NextResponse.redirect(publicUrl(request, localizedAppPath(request, pathname)), { status: 303 });
}

function redirectToDetail(request: Request, strategyId: string, suffix = "") {
  return redirectToApp(request, `/strategies/${strategyId}${suffix}`);
}

function redirectToPublic(request: Request, pathname: string) {
  return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, pathname)), { status: 303 });
}
