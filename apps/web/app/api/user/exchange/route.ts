import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);

  if (!sessionToken) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  if (intent === "save") {
    const apiKey = readField(formData, "apiKey");
    const apiSecret = readField(formData, "apiSecret");
    const positionMode = readField(formData, "positionMode") || "hedge";
    const result = await exchangePost(sessionToken, "/exchange/binance/credentials", {
      api_key: apiKey,
      api_secret: apiSecret,
      expected_hedge_mode: positionMode === "hedge",
      selected_markets: ["spot", "usdm", "coinm"],
    });

    if (!result.ok) {
      return redirectWithError(request, result.error ?? "Exchange credential save failed");
    }

    return NextResponse.redirect(new URL("/app/exchange?exchange=credentials-saved", request.url), { status: 303 });
  }

  const account = await exchangeGet(sessionToken, "/exchange/binance/account");
  if (!account.ok) {
    return redirectWithError(request, account.error ?? "Connection test failed");
  }

  const status = account.data?.account?.connection_status === "healthy" ? "test-passed" : "test-failed";
  return NextResponse.redirect(new URL(`/app/exchange?exchange=${status}`, request.url), { status: 303 });
}

async function exchangePost(sessionToken: string, path: string, body: Record<string, unknown>) {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
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

  return { ok: true as const, data: await response.json() };
}

async function exchangeGet(sessionToken: string, path: string) {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "GET",
    headers: {
      authorization: `Bearer ${sessionToken}`,
    },
    cache: "no-store",
  });

  if (!response.ok) {
    return { ok: false as const, error: await readError(response) };
  }

  return { ok: true as const, data: await response.json() };
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "exchange request failed";
  } catch {
    return "exchange request failed";
  }
}

function redirectWithError(request: Request, error: string) {
  return NextResponse.redirect(new URL(`/app/exchange?error=${encodeURIComponent(error)}`, request.url), { status: 303 });
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
