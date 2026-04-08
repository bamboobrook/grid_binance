import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const ids = formData.getAll("ids").filter((value): value is string => typeof value === "string").map((value) => value.trim()).filter(Boolean);
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  if (intent === "stop-all") {
    const response = await fetch(`${authApiBaseUrl()}/strategies/stop-all`, {
      method: "POST",
      headers: { authorization: `Bearer ${sessionToken}` },
      cache: "no-store",
    });
    if (!response.ok) {
      return redirectWithError(request, await readError(response));
    }
    const payload = (await response.json()) as { stopped?: number };
    if ((payload.stopped ?? 0) === 0) {
      return redirectWithError(request, "No running strategies were stopped.");
    }
    return NextResponse.redirect(new URL("/app/strategies?notice=stop-all-complete", request.url), { status: 303 });
  }

  if (ids.length === 0) {
    return redirectWithError(request, "Select at least one strategy.");
  }

  const path = intent === "start" ? "/strategies/batch/start" : intent === "pause" ? "/strategies/batch/pause" : intent === "delete" ? "/strategies/batch/delete" : null;
  if (!path) {
    return redirectWithError(request, "Unknown batch strategy action.");
  }

  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ ids }),
    cache: "no-store",
  });
  if (!response.ok) {
    return redirectWithError(request, await readError(response));
  }

  const payload = (await response.json()) as { started?: number; paused?: number; deleted?: number; failures?: Array<{ error?: string }> };
  const changed = intent === "start" ? payload.started ?? 0 : intent === "pause" ? payload.paused ?? 0 : payload.deleted ?? 0;
  if (changed === 0) {
    if (intent === "start" && payload.failures?.[0]?.error) {
      return redirectWithError(request, payload.failures[0].error);
    }
    return redirectWithError(request, intent === "start" ? "No selected strategy could be started." : intent === "pause" ? "No running strategy was paused." : "Selected strategies could not be deleted.");
  }

  return NextResponse.redirect(new URL(`/app/strategies?notice=batch-${intent}-complete`, request.url), { status: 303 });
}

function redirectWithError(request: Request, error: string) {
  return NextResponse.redirect(new URL(`/app/strategies?error=${encodeURIComponent(error)}`, request.url), { status: 303 });
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "strategy request failed";
  } catch {
    return "strategy request failed";
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
