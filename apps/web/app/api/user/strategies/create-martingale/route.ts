import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../../lib/auth";

export async function POST(request: Request) {
  const formData = await request.formData();
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return redirectPublic(request, "/login?error=session+expired");
  }

  const payload = buildMartingalePayload(formData);
  if (!payload.symbol) {
    return redirectApp(request, `/strategies/new?strategyType=martingale_grid&error=${encodeURIComponent("请选择交易对。")}`);
  }

  const query = new URLSearchParams({
    notice: "preview-martingale-create",
    symbol: payload.symbol,
    view: "cards",
  });
  return redirectApp(request, `/strategies?${query.toString()}`);
}

function buildMartingalePayload(formData: FormData) {
  return {
    symbol: readField(formData, "symbol"),
    market_type: readField(formData, "marketType") || "spot",
    direction: readField(formData, "martingaleDirection") || "long",
    first_order_quote: readField(formData, "martingaleFirstOrderQuote") || "25",
    safety_order_step_pct: readField(formData, "martingaleSpacingPercent") || "1.2",
    order_multiplier: readField(formData, "martingaleOrderMultiplier") || "1.6",
    max_safety_orders: readField(formData, "martingaleMaxLegs") || "6",
    take_profit_pct: readField(formData, "martingaleTakeProfitPercent") || "1.4",
    stop_loss_pct: readField(formData, "martingaleStopLossPercent"),
    margin_mode: readField(formData, "futuresMarginMode") || "isolated",
    leverage: readField(formData, "leverage") || "1",
    name: readField(formData, "name"),
    notes: readField(formData, "notes"),
  };
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function redirectApp(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedAppPath(request, path)), { status: 303 });
}

function redirectPublic(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, path)), { status: 303 });
}
