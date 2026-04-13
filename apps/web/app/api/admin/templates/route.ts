import { authApiBaseUrl } from "../../../../lib/api/admin-product-state";

import { postAdminBackend, proxyAdminBackendError, readField, readSessionToken, redirectTo } from "../_shared";

function readBoolField(formData: FormData, key: string) {
  return readField(formData, key) === "true";
}

function readOptionalNumberField(formData: FormData, key: string) {
  const value = readField(formData, key);
  if (!value) {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function readTemplateLevels(formData: FormData) {
  const raw = readField(formData, "levelsJson");
  if (raw) {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) {
      throw new Error("levelsJson must be a non-empty array");
    }
    return parsed.map((level) => ({
      entry_price: String(level.entry_price ?? "").trim(),
      quantity: String(level.quantity ?? "").trim(),
      take_profit_bps: Number(level.take_profit_bps ?? 0),
      trailing_bps: level.trailing_bps === null || level.trailing_bps === undefined || level.trailing_bps === "" ? null : Number(level.trailing_bps),
    }));
  }
  return [
    {
      entry_price: readField(formData, "level1EntryPrice"),
      quantity: readField(formData, "level1Quantity"),
      take_profit_bps: Number(readField(formData, "level1TakeProfitBps") || "0"),
      trailing_bps: readOptionalNumberField(formData, "level1TrailingBps"),
    },
    {
      entry_price: readField(formData, "level2EntryPrice"),
      quantity: readField(formData, "level2Quantity"),
      take_profit_bps: Number(readField(formData, "level2TakeProfitBps") || "0"),
      trailing_bps: readOptionalNumberField(formData, "level2TrailingBps"),
    },
  ];
}

function buildTemplatePayload(formData: FormData) {
  const market = readField(formData, "market") || "Spot";
  return {
    name: readField(formData, "name"),
    symbol: readField(formData, "symbol"),
    market,
    mode: readField(formData, "mode") || "SpotClassic",
    generation: readField(formData, "generation") || "Custom",
    amount_mode: readField(formData, "amountMode") === "base" ? "Base" : "Quote",
    futures_margin_mode: market === "Spot" ? null : (readField(formData, "futuresMarginMode") === "cross" ? "Cross" : "Isolated"),
    leverage: market === "Spot" ? null : readOptionalNumberField(formData, "leverage"),
    levels: readTemplateLevels(formData),
    membership_ready: readBoolField(formData, "membershipReady"),
    exchange_ready: readBoolField(formData, "exchangeReady"),
    permissions_ready: readBoolField(formData, "permissionsReady"),
    withdrawals_disabled: readBoolField(formData, "withdrawalsDisabled"),
    hedge_mode_ready: readBoolField(formData, "hedgeModeReady"),
    symbol_ready: readBoolField(formData, "symbolReady"),
    filters_ready: readBoolField(formData, "filtersReady"),
    margin_ready: readBoolField(formData, "marginReady"),
    conflict_ready: readBoolField(formData, "conflictReady"),
    balance_ready: readBoolField(formData, "balanceReady"),
    overall_take_profit_bps: readOptionalNumberField(formData, "overallTakeProfitBps"),
    overall_stop_loss_bps: readOptionalNumberField(formData, "overallStopLossBps"),
    strategy_type: readField(formData, "strategyType") || "ordinary_grid",
    reference_price_source: readField(formData, "referencePriceSource") || "manual",
    reference_price: readField(formData, "referencePrice") || null,
    post_trigger_action: readField(formData, "postTriggerAction") || "Stop",
  };
}

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const name = readField(formData, "name");

  if (intent === "update") {
    const templateId = readField(formData, "templateId");
    const sessionToken = readSessionToken(request) ?? "";
    const response = await fetch(`${authApiBaseUrl()}/admin/templates/${templateId}`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${sessionToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(buildTemplatePayload(formData)),
      cache: "no-store",
    });

    if (!response.ok) {
      return proxyAdminBackendError(response);
    }

    return redirectTo(request, `/admin/templates?updated=${encodeURIComponent(name)}`);
  }

  const response = await postAdminBackend(request, "/admin/templates", buildTemplatePayload(formData));
  if (!response.ok) {
    return proxyAdminBackendError(response);
  }
  return redirectTo(request, `/admin/templates?created=${encodeURIComponent(name)}`);
}
