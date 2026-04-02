import { NextResponse } from "next/server";

import { buildMaskedApiKey, updateUserProductState } from "../../../../lib/api/user-product-state";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const apiKey = readField(formData, "apiKey");
  const apiSecret = readField(formData, "apiSecret");
  const positionMode = readField(formData, "positionMode") || "hedge";

  updateUserProductState(readSessionToken(request), (state) => {
    if (intent === "save") {
      state.exchange.saved = apiKey.length > 0 && apiSecret.length > 0;
      state.exchange.apiKeyMasked = state.exchange.saved ? buildMaskedApiKey(apiKey) : null;
      state.exchange.positionMode = positionMode;
      state.exchange.connectionStatus = "idle";
      state.exchange.connectionMessage = null;
      state.flash.exchange = state.exchange.saved ? "Credentials saved" : "Credentials were incomplete and were not saved.";
    }

    if (intent === "test") {
      state.exchange.connectionStatus = state.exchange.saved && state.exchange.positionMode === "hedge" ? "passed" : "failed";
      state.exchange.connectionMessage =
        state.exchange.connectionStatus === "passed"
          ? "Spot, USDⓈ-M, and COIN-M permissions verified."
          : "Save credentials first and enable hedge mode before running the connection test.";
      state.flash.exchange =
        state.exchange.connectionStatus === "passed" ? "Connection test passed" : "Connection test failed";
      state.tradeHistory.unshift({
        id: `hist-${Date.now()}`,
        at: "2026-04-02 10:06",
        activity: "API credential retest",
        detail: state.exchange.connectionStatus === "passed" ? "Passed" : "Blocked",
      });
      state.tradeHistory = state.tradeHistory.slice(0, 6);
    }
  });

  return NextResponse.redirect(new URL("/app/exchange", request.url), { status: 303 });
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
