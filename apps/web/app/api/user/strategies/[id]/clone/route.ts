import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return NextResponse.json({ error: "session expired" }, { status: 401 });
  }

  const response = await fetch(`${authApiBaseUrl()}/strategies/${id}/clone`, {
    method: "POST",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });

  if (!response.ok) {
    return NextResponse.json({ error: await readError(response) }, { status: response.status });
  }

  const created = (await response.json()) as { id: string };
  const locale = readLocale(request);
  return NextResponse.redirect(new URL(`/${locale}/app/strategies/${created.id}?notice=cloned`, request.url), { status: 303 });
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "clone failed";
  } catch {
    return "clone failed";
  }
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function readLocale(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )NEXT_LOCALE=([^;]+)/);
  return match ? match[1] : "zh";
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
