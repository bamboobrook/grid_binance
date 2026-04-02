import { NextResponse } from "next/server";

import {
  findStrategy,
  membershipAllowsNewStarts,
  updateUserProductState,
} from "../../../../../lib/api/user-product-state";

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  const formData = await request.formData();
  const intent = readField(formData, "intent");

  updateUserProductState(readSessionToken(request), (state) => {
    const strategy = findStrategy(state, id);
    if (!strategy) {
      return;
    }

    if (intent === "save") {
      strategy.name = readField(formData, "name") || strategy.name;
      strategy.symbol = readField(formData, "symbol") || strategy.symbol;
      strategy.marketType = readField(formData, "marketType") || strategy.marketType;
      strategy.trailingTakeProfit = readField(formData, "trailing") || strategy.trailingTakeProfit;
      strategy.postTrigger = readField(formData, "postTrigger") || strategy.postTrigger;
      strategy.preflightStatus = "idle";
      strategy.preflightMessage = null;
      if (strategy.status === "running") {
        strategy.status = "paused";
      }
      state.flash.strategy = "Edits saved";
    }

    if (intent === "preflight") {
      const membershipReady = membershipAllowsNewStarts(state.billing.membershipStatus);
      const exchangeReady = state.exchange.saved && state.exchange.connectionStatus === "passed";
      const hedgeReady = strategy.marketType === "spot" || state.exchange.positionMode === "hedge";
      strategy.preflightChecks = [
        { id: `${id}-check-1`, item: "membership_status", result: membershipReady ? "Pass" : "Fail" },
        { id: `${id}-check-2`, item: "Exchange filters", result: exchangeReady ? "Pass" : "Fail" },
        { id: `${id}-check-3`, item: "Balance coverage", result: "Pass" },
        { id: `${id}-check-4`, item: "Hedge mode", result: hedgeReady ? "Pass" : "Fail" },
      ];
      strategy.preflightStatus = membershipReady && exchangeReady && hedgeReady ? "passed" : "failed";
      strategy.preflightMessage =
        strategy.preflightStatus === "passed"
          ? "Exchange filters, balance, and hedge-mode checks passed."
          : !membershipReady
            ? "Pre-flight failed. Renew or reactivate membership before starting this strategy."
            : "Pre-flight failed. Save/test exchange credentials and confirm hedge mode before starting.";
      strategy.status = strategy.preflightStatus === "passed" ? "ready" : strategy.status;
      state.flash.strategy = strategy.preflightStatus === "passed" ? "Pre-flight passed" : "Pre-flight failed";
    }

    if (intent === "start") {
      if (!membershipAllowsNewStarts(state.billing.membershipStatus)) {
        strategy.preflightStatus = "failed";
        strategy.preflightMessage = "Start blocked. Membership is not active or in grace.";
        state.flash.strategy = "Start blocked until membership is active or in grace";
        return;
      }

      if (strategy.preflightStatus === "passed") {
        strategy.status = "running";
        state.flash.strategy = "Strategy started";
      } else {
        state.flash.strategy = "Start blocked until pre-flight passes";
      }
    }
  });

  return NextResponse.redirect(new URL(`/app/strategies/${id}`, request.url), { status: 303 });
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
