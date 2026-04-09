import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const SESSION_TOKEN_COOKIE = "session_token";
const UI_LANGUAGE_COOKIE = "ui_lang";

type ErrorPayload = {
  error?: string;
};

export type LoginResponse = {
  session_token: string;
};

export type RegisterResponse = {
  user_id: number;
  code_delivery: string;
  verification_code?: string;
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

  return readAuthApiResponse<ResponseBody>(response);
}

export async function authApiGet<ResponseBody>(
  path: string,
  sessionToken: string,
): Promise<ResponseBody> {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "GET",
    headers: {
      authorization: `Bearer ${sessionToken}`,
    },
    cache: "no-store",
  });

  return readAuthApiResponse<ResponseBody>(response);
}

async function readAuthApiResponse<ResponseBody>(response: Response): Promise<ResponseBody> {
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

export function publicUrl(request: Request, pathname: string) {
  const normalizedPath = localizedPath(request, pathname);
  const forwardedProto = request.headers.get("x-forwarded-proto")?.trim() || "http";
  const forwardedHost = request.headers.get("x-forwarded-host")?.trim();
  const forwardedPort = request.headers.get("x-forwarded-port")?.trim();

  if (forwardedHost) {
    const includePort =
      !!forwardedPort &&
      !forwardedHost.includes(":") &&
      !((forwardedProto === "http" && forwardedPort === "80") ||
        (forwardedProto === "https" && forwardedPort === "443"));
    const base = includePort
      ? `${forwardedProto}://${forwardedHost}:${forwardedPort}`
      : `${forwardedProto}://${forwardedHost}`;
    return new URL(normalizedPath, base);
  }

  const host = request.headers.get("host")?.trim();
  if (host) {
    const current = new URL(request.url);
    return new URL(normalizedPath, `${current.protocol}//${host}`);
  }

  return new URL(normalizedPath, request.url);
}

export function requestLocale(request: Request): "zh" | "en" {
  const requestUrl = new URL(request.url);
  const candidates = [
    requestUrl.searchParams.get("locale"),
    localeFromPath(requestUrl.pathname),
    localeFromReferer(request.headers.get("referer")),
    localeFromCookie(request.headers.get("cookie")),
  ];

  return candidates.find((value): value is "zh" | "en" => value === "zh" || value === "en") ?? "zh";
}

export function localizedPath(request: Request, pathname: string) {
  if (!pathname.startsWith("/")) {
    return pathname;
  }

  const [basePath, suffix = ""] = pathname.split(/(?=[?#])/, 2);
  if (isLocalizedPath(basePath) || !needsLocalePrefix(basePath)) {
    return pathname;
  }

  const locale = requestLocale(request);
  if (basePath === "/") {
    return `/${locale}${suffix}`;
  }

  return `/${locale}${basePath}${suffix}`;
}

export function localizedPublicPath(request: Request, pathname: string) {
  return localizedPath(request, pathname);
}

export function localizedAppPath(request: Request, pathname = "/dashboard") {
  return localizedPath(request, `/app${normalizeSubpath(pathname)}`);
}

export function localizedAdminPath(request: Request, pathname = "/dashboard") {
  return localizedPath(request, `/admin${normalizeSubpath(pathname)}`);
}

function normalizeSubpath(pathname: string) {
  if (pathname === "") {
    return "";
  }
  return pathname.startsWith("/") ? pathname : `/${pathname}`;
}

function needsLocalePrefix(pathname: string) {
  return pathname === "/" || /^\/(?:app|admin|login|register|password-reset|verify-email|admin-bootstrap)(?:\/|$)/.test(pathname);
}

function isLocalizedPath(pathname: string) {
  return /^\/(?:zh|en)(?:\/|$)/.test(pathname);
}

function localeFromPath(pathname: string) {
  const match = pathname.match(/^\/(zh|en)(?:\/|$)/);
  return match?.[1] ?? null;
}

function localeFromReferer(referer: string | null) {
  if (!referer) {
    return null;
  }

  try {
    return localeFromPath(new URL(referer).pathname);
  } catch {
    return null;
  }
}

function localeFromCookie(cookieHeader: string | null) {
  if (!cookieHeader) {
    return null;
  }

  const match = cookieHeader.match(new RegExp(`(?:^|; )${UI_LANGUAGE_COOKIE}=([^;]+)`));
  const value = match ? decodeURIComponent(match[1]) : null;
  return value === "en" || value === "zh" ? value : null;
}

export function shouldUseSecureCookie(request: Request) {
  const forwardedProto = request.headers.get("x-forwarded-proto")?.trim();
  if (forwardedProto) {
    return forwardedProto === "https";
  }
  return new URL(request.url).protocol === "https:";
}

export function buildSessionRedirect(
  request: Request,
  nextPath: string | null | undefined,
  sessionToken: string,
  fallbackPath = localizedAppPath(request, "/dashboard"),
) {
  const response = NextResponse.redirect(
    publicUrl(request, safeRedirectTarget(nextPath, fallbackPath)),
    { status: 303 },
  );

  response.cookies.set(SESSION_TOKEN_COOKIE, sessionToken, {
    httpOnly: true,
    path: "/",
    sameSite: "lax",
    secure: shouldUseSecureCookie(request),
  });

  return response;
}

export function buildErrorRedirect(
  request: Request,
  pathname: "/login" | "/register",
  params: {
    email?: string;
    next?: string | null;
    error: string;
    extra?: Record<string, string>;
  },
) {
  const url = publicUrl(request, localizedPublicPath(request, pathname));
  if (params.email) {
    url.searchParams.set("email", params.email);
  }
  if (params.next) {
    url.searchParams.set("next", localizedPath(request, safeRedirectTarget(params.next, localizedAppPath(request, "/dashboard"))));
  }
  url.searchParams.set("error", params.error);

  for (const [key, value] of Object.entries(params.extra ?? {})) {
    url.searchParams.set(key, value);
  }

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
