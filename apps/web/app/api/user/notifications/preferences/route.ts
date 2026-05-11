import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return NextResponse.json({ error: "session expired" }, { status: 401 });
  }

  const profileResponse = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!profileResponse.ok) {
    return NextResponse.json({ error: "session expired" }, { status: 401 });
  }
  const profile = (await profileResponse.json()) as { email?: string };
  const email = profile.email;
  if (!email) {
    return NextResponse.json({ error: "session expired" }, { status: 401 });
  }

  const formData = await request.formData();
  const prefs = {
    email,
    take_profit: formData.has("take_profit"),
    stop_loss: formData.has("stop_loss"),
    error: formData.has("error"),
    daily_report: formData.has("daily_report"),
  };

  const response = await fetch(`${authApiBaseUrl()}/notifications/preferences`, {
    method: "PUT",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(prefs),
    cache: "no-store",
  });

  if (!response.ok) {
    return NextResponse.json({ error: await readError(response) }, { status: response.status });
  }

  const referer = request.headers.get("referer") || "/app/notifications";
  return NextResponse.redirect(new URL(referer), { status: 303 });
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "save failed";
  } catch {
    return "save failed";
  }
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}