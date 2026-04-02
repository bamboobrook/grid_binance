import { NextResponse } from "next/server";

import { createStrategyRecord, updateUserProductState } from "../../../../../lib/api/user-product-state";

export async function POST(request: Request) {
  const formData = await request.formData();
  const strategy = createStrategyRecord({
    generation: readField(formData, "generation") || "geometric",
    marketType: readField(formData, "marketType") || "spot",
    mode: readField(formData, "mode") || "classic",
    name: readField(formData, "name") || "Strategy Draft",
    postTrigger: readField(formData, "postTrigger") || "rebuild",
    symbol: readField(formData, "symbol") || "BTCUSDT",
    trailingTakeProfit: readField(formData, "trailing") || "0.8",
  });

  updateUserProductState(readSessionToken(request), (state) => {
    state.strategies = state.strategies.filter((item) => item.id !== strategy.id);
    state.strategies.unshift(strategy);
    state.flash.strategy = "Draft saved";
  });

  return NextResponse.redirect(new URL(`/app/strategies/${strategy.id}`, request.url), { status: 303 });
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
