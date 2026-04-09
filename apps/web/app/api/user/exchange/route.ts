import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const EXCHANGE_TEST_RESULT_COOKIE = "exchange_test_result";

type ExchangeTestResponse = {
  account?: {
    connection_status?: string;
  };
  synced_symbols?: number;
  persisted?: boolean;
};

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);

  if (!sessionToken) {
    return redirectPublic(request, "/login?error=session+expired");
  }

  const apiKey = readField(formData, "apiKey");
  const apiSecret = readField(formData, "apiSecret");
  const positionMode = readField(formData, "positionMode") || "hedge";
  const selectedMarkets = readFields(formData, "selectedMarkets");
  const payload = {
    api_key: apiKey,
    api_secret: apiSecret,
    expected_hedge_mode: positionMode === "hedge",
    selected_markets: selectedMarkets,
  };

  if (intent === "save") {
    return persistCredentials(request, sessionToken, payload);
  }

  if (intent === "test") {
    const result = await exchangePost<ExchangeTestResponse>(
      sessionToken,
      "/exchange/binance/credentials/test",
      payload,
    );

    if (!result.ok) {
      return redirectExchangeError(request, result.error ?? "Connection test failed");
    }

    if (result.data?.account?.connection_status === "healthy") {
      const persisted = await exchangePost<ExchangeTestResponse>(
        sessionToken,
        "/exchange/binance/credentials",
        payload,
      );

      if (!persisted.ok) {
        const response = redirectExchangeError(
          request,
          `Connection test passed, but auto-save failed: ${persisted.error ?? "Exchange credential save failed"}`,
        );
        setTestResultCookie(response, {
          ...(result.data ?? {}),
          persisted: false,
        });
        return response;
      }

      const response = redirectApp(request, "/exchange?exchange=test-passed-saved");
      response.cookies.delete(EXCHANGE_TEST_RESULT_COOKIE);
      return response;
    }

    const response = redirectApp(request, "/exchange?exchange=test-failed");
    setTestResultCookie(response, result.data ?? {});
    return response;
  }

  return redirectExchangeError(request, "Unsupported exchange action");
}

async function persistCredentials(request: Request, sessionToken: string, payload: Record<string, unknown>) {
  const result = await exchangePost(sessionToken, "/exchange/binance/credentials", payload);

  if (!result.ok) {
    return redirectExchangeError(request, result.error ?? "Exchange credential save failed");
  }

  const response = redirectApp(request, "/exchange?exchange=credentials-saved");
  response.cookies.delete(EXCHANGE_TEST_RESULT_COOKIE);
  return response;
}

function redirectExchangeError(request: Request, message: string) {
  const response = redirectApp(request, `/exchange?error=${encodeURIComponent(message)}`);
  response.cookies.delete(EXCHANGE_TEST_RESULT_COOKIE);
  return response;
}

function setTestResultCookie(response: NextResponse, payload: ExchangeTestResponse) {
  response.cookies.set(EXCHANGE_TEST_RESULT_COOKIE, encodeURIComponent(JSON.stringify(payload)), {
    httpOnly: true,
    maxAge: 600,
    path: "/",
    sameSite: "lax",
  });
}

async function exchangePost<T>(sessionToken: string, path: string, body: Record<string, unknown>) {
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

  return { ok: true as const, data: (await response.json()) as T };
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "exchange request failed";
  } catch {
    return "exchange request failed";
  }
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function readFields(formData: FormData, key: string) {
  return formData
    .getAll(key)
    .map((value) => (typeof value === "string" ? value.trim() : ""))
    .filter((value, index, values) => value.length > 0 && values.indexOf(value) === index);
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
