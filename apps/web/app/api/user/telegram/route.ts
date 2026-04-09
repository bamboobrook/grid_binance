import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);

  if (!sessionToken) {
    return redirectPublic(request, "/login?error=session+expired");
  }

  const profile = await fetchProfile(sessionToken);
  if (!profile) {
    return redirectPublic(request, "/login?error=session+expired");
  }

  if (intent !== "generate") {
    return redirectApp(request, "/telegram");
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
    return redirectApp(request, `/telegram?error=${encodeURIComponent(error)}`);
  }

  const payload = (await response.json()) as { code: string; expires_at: string };
  const url = publicUrl(request, localizedAppPath(request, "/telegram"));
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

function redirectApp(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedAppPath(request, path)), { status: 303 });
}

function redirectPublic(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, path)), { status: 303 });
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
