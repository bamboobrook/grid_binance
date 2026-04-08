import { NextResponse } from "next/server";
import { localizedPath, localizedPublicPath, publicUrl } from "@/lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const planCode = (readField(formData, "plan") || "monthly").toLowerCase();
  const chain = normalizeChain(readField(formData, "chain") || "bsc");
  const asset = normalizeAsset(readField(formData, "token") || "usdt");
  const sessionToken = readSessionToken(request);
  const profile = sessionToken ? await fetchProfile(sessionToken) : null;

  if (!sessionToken || !profile?.email) {
    return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?error=session+expired")), { status: 303 });
  }

  const result = await createBillingOrder(sessionToken, {
    email: profile.email,
    chain,
    asset,
    plan_code: planCode,
    requested_at: new Date().toISOString(),
  });

  if (!result.ok) {
    return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/billing?error=${encodeURIComponent(result.error)}`)), { status: 303 });
  }

  const chainLabel = humanChainLabel(result.data.chain);
  const notice = result.data.address
    ? `Send exactly ${result.data.amount} ${result.data.asset} on ${chainLabel} to ${result.data.address}. Address lock expires ${result.data.expires_at ?? "soon"}. Overpayment, underpayment, or wrong token will require manual review.`
    : `Order queued for ${chainLabel} ${result.data.asset}. Exact amount ${result.data.amount} remains reserved while awaiting address assignment. Queue position ${result.data.queue_position ?? "pending"}.`;
  return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/billing?notice=${encodeURIComponent(notice)}`)), { status: 303 });
}

async function fetchProfile(sessionToken: string) {
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as { email?: string };
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
      expires_at?: string | null;
      order_id: number;
      queue_position?: number | null;
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
