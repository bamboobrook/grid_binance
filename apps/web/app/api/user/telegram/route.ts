import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);

  if (!sessionToken) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  const profile = await fetchProfile(sessionToken);
  if (!profile) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  if (intent !== "generate") {
    return NextResponse.redirect(new URL("/app/telegram", request.url), { status: 303 });
  }

  const response = await fetch(`${authApiBaseUrl()}/telegram/bind-codes`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ email: profile.email, ttl_seconds: 300 }),
    cache: "no-store",
  });

  if (!response.ok) {
    const error = await readError(response);
    return NextResponse.redirect(new URL(`/app/telegram?error=${encodeURIComponent(error)}`, request.url), { status: 303 });
  }

  const payload = (await response.json()) as { code: string; expires_at: string };
  const url = new URL("/app/telegram", request.url);
  url.searchParams.set("notice", "bind-code-issued");
  url.searchParams.set("code", payload.code);
  url.searchParams.set("expires", payload.expires_at);
  return NextResponse.redirect(url, { status: 303 });
}

async function fetchProfile(sessionToken: string) {
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: {
      authorization: `Bearer ${sessionToken}`,
    },
    cache: "no-store",
  });

  if (!response.ok) {
    return null;
  }

  const payload = (await response.json()) as { email?: string };
  if (typeof payload.email !== "string") {
    return null;
  }

  return payload;
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "telegram request failed";
  } catch {
    return "telegram request failed";
  }
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
