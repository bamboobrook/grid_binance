import { NextResponse } from "next/server";

import { fetchBackendTruth, updateUserProductState } from "../../../../lib/api/user-product-state";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const planCode = (readField(formData, "plan") || "monthly").toLowerCase();
  const chain = normalizeChain(readField(formData, "chain") || "bsc");
  const asset = normalizeAsset(readField(formData, "token") || "usdt");
  const sessionToken = readSessionToken(request);
  const backendTruth = sessionToken ? await fetchBackendTruth(sessionToken) : null;

  if (!sessionToken || !backendTruth?.profile) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  const result = await createBillingOrder(sessionToken, {
    email: backendTruth.profile.email,
    chain,
    asset,
    plan_code: planCode,
    requested_at: new Date().toISOString(),
  });

  updateUserProductState(sessionToken, (state) => {
    if (!result.ok) {
      state.flash.billing = `Billing order failed: ${result.error}`;
      return;
    }

    const chainLabel = humanChainLabel(result.data.chain);
    const nextOrder = {
      id: String(result.data.order_id),
      order: `ORD-${String(result.data.order_id).padStart(4, "0")}`,
      chain: chainLabel,
      token: result.data.asset,
      amount: result.data.amount,
      state: result.data.address ? "Awaiting exact transfer" : "Queued for address assignment",
    };
    state.billing.orders = [nextOrder, ...state.billing.orders.filter((order) => order.id !== nextOrder.id)];
    state.flash.billing = result.data.address
      ? `Send exactly ${result.data.amount} ${result.data.asset} on ${chainLabel}. Overpayment, underpayment, or wrong token will require manual review.`
      : `Order queued for ${chainLabel} ${result.data.asset}. Exact amount ${result.data.amount} remains reserved while awaiting address assignment.`;
  });

  return NextResponse.redirect(new URL("/app/billing", request.url), { status: 303 });
}

async function createBillingOrder(
  sessionToken: string,
  body: {
    asset: string;
    chain: string;
    email: string;
    plan_code: string;
    requested_at: string;
  },
) {
  const response = await fetch(`${authApiBaseUrl()}/billing/orders`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });

  if (!response.ok) {
    return { ok: false as const, error: await readError(response) };
  }

  return {
    ok: true as const,
    data: (await response.json()) as {
      address?: string | null;
      amount: string;
      asset: string;
      chain: string;
      order_id: number;
    },
  };
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "billing request failed";
  } catch {
    return "billing request failed";
  }
}

function normalizeChain(value: string) {
  switch (value.trim().toLowerCase()) {
    case "ethereum":
    case "eth":
      return "ETH";
    case "solana":
    case "sol":
      return "SOL";
    default:
      return "BSC";
  }
}

function humanChainLabel(value: string) {
  switch (value.trim().toUpperCase()) {
    case "ETH":
      return "Ethereum";
    case "SOL":
      return "Solana";
    default:
      return "BSC";
  }
}

function normalizeAsset(value: string) {
  return value.trim().toUpperCase() === "USDC" ? "USDC" : "USDT";
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
