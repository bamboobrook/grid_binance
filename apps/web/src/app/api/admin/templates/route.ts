import { postAdminBackend, readField, redirectTo } from "../_shared";

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

export async function POST(request: Request) {
  const formData = await request.formData();
  const name = readField(formData, "name");

  await postAdminBackend(request, "/admin/templates", {
    name,
    symbol: readField(formData, "symbol"),
    market: readField(formData, "market") || "Spot",
    mode: readField(formData, "mode") || "SpotClassic",
    generation: readField(formData, "generation") || "Custom",
    levels: [
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
    ],
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
    post_trigger_action: readField(formData, "postTriggerAction") || "Stop",
  });

  return redirectTo(request, `/admin/templates?created=${encodeURIComponent(name)}`);
}
