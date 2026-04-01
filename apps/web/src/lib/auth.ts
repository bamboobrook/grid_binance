import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const SESSION_TOKEN_COOKIE = "session_token";

type ErrorPayload = {
  error?: string;
};

export type LoginResponse = {
  session_token: string;
};

export type RegisterResponse = {
  user_id: number;
  verification_code: string;
};

export class AuthProxyError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "AuthProxyError";
    this.status = status;
  }
}

export async function authApiPost<ResponseBody>(
  path: string,
  body: Record<string, string | null>,
): Promise<ResponseBody> {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });

  if (!response.ok) {
    let message = "auth request failed";

    try {
      const payload = (await response.json()) as ErrorPayload;
      if (typeof payload.error === "string" && payload.error.trim() !== "") {
        message = payload.error;
      }
    } catch {
      // Keep the generic message when the backend does not return JSON.
    }

    throw new AuthProxyError(message, response.status);
  }

  return (await response.json()) as ResponseBody;
}

export function buildSessionRedirect(
  requestUrl: string,
  nextPath: string | null | undefined,
  sessionToken: string,
) {
  const response = NextResponse.redirect(
    new URL(safeRedirectTarget(nextPath, "/app/dashboard"), requestUrl),
    { status: 303 },
  );

  response.cookies.set(SESSION_TOKEN_COOKIE, sessionToken, {
    httpOnly: true,
    path: "/",
    sameSite: "lax",
    secure: false,
  });

  return response;
}

export function buildErrorRedirect(
  requestUrl: string,
  pathname: "/login" | "/register",
  params: {
    email?: string;
    next?: string | null;
    error: string;
  },
) {
  const url = new URL(pathname, requestUrl);
  if (params.email) {
    url.searchParams.set("email", params.email);
  }
  if (params.next) {
    url.searchParams.set("next", safeRedirectTarget(params.next, "/app/dashboard"));
  }
  url.searchParams.set("error", params.error);

  return NextResponse.redirect(url, { status: 303 });
}

export function firstValue(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}

export function safeRedirectTarget(
  candidate: string | null | undefined,
  fallback: string,
) {
  if (!candidate) {
    return fallback;
  }

  return candidate.startsWith("/") && !candidate.startsWith("//")
    ? candidate
    : fallback;
}

function authApiBaseUrl() {
  return (
    process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") ||
    DEFAULT_AUTH_API_BASE_URL
  );
}
