import { postAdminBackend, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const name = readField(formData, "name");

  await postAdminBackend(request, "/admin/templates", {
    name,
    symbol: "ADAUSDT",
    market: "Spot",
    mode: "SpotClassic",
    generation: "Custom",
    levels: [
      { entry_price: "1.00", quantity: "10", take_profit_bps: 150, trailing_bps: null },
      { entry_price: "1.10", quantity: "10", take_profit_bps: 180, trailing_bps: null },
    ],
    membership_ready: true,
    exchange_ready: true,
    permissions_ready: true,
    withdrawals_disabled: true,
    hedge_mode_ready: true,
    symbol_ready: true,
    filters_ready: true,
    margin_ready: true,
    conflict_ready: true,
    balance_ready: true,
    overall_take_profit_bps: null,
    overall_stop_loss_bps: null,
    post_trigger_action: "Stop",
  });

  return redirectTo(request, `/admin/templates?created=${encodeURIComponent(name)}`);
}
