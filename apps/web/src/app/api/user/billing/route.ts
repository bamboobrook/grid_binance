import { NextResponse } from "next/server";

import { updateUserProductState } from "../../../../lib/api/user-product-state";

const amounts: Record<string, string> = {
  monthly: "20.00",
  quarterly: "54.00",
  yearly: "180.00",
};

export async function POST(request: Request) {
  const formData = await request.formData();
  const plan = readField(formData, "plan") || "monthly";
  const chain = readField(formData, "chain") || "bsc";
  const token = readField(formData, "token") || "usdt";

  updateUserProductState(readSessionToken(request), (state) => {
    const chainLabel = chain === "ethereum" ? "Ethereum" : chain === "solana" ? "Solana" : "BSC";
    const tokenLabel = token.toUpperCase();
    state.billing.orders.unshift({
      id: `order-${Date.now()}`,
      order: `ORD-${String(Date.now()).slice(-4)}`,
      chain: chainLabel,
      token: tokenLabel,
      amount: amounts[plan] ?? amounts.monthly,
      state: "Awaiting exact transfer",
    });
    state.flash.billing = `Send exactly ${amounts[plan] ?? amounts.monthly} ${tokenLabel} on ${chainLabel}. Overpayment, underpayment, or wrong token will require manual review.`;
  });

  return NextResponse.redirect(new URL("/app/billing", request.url), { status: 303 });
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
