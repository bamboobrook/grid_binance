import { NextResponse } from "next/server";

import { publicUrl, shouldUseSecureCookie } from "@/lib/auth";
import {
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
  type UiLanguage,
  type UiTheme,
} from "@/lib/ui/preferences";

const ONE_YEAR_SECONDS = 60 * 60 * 24 * 365;

export async function POST(request: Request) {
  const formData = await request.formData();
  const nextLanguage = readLanguage(formData.get("lang"));
  const nextTheme = readTheme(formData.get("theme"));
  const formReturnTo = safeReturnPath(readField(formData, "returnTo"));
  const refererReturnTo = safeRefererPath(request);
  const returnTo = formReturnTo.includes("?") || !refererReturnTo ? formReturnTo : refererReturnTo;
  const redirectPath = nextLanguage ? withLocale(returnTo, nextLanguage) : returnTo;
  const response = NextResponse.redirect(publicUrl(request, redirectPath), { status: 303 });
  const secure = shouldUseSecureCookie(request);

  if (nextLanguage) {
    response.cookies.set(UI_LANGUAGE_COOKIE, nextLanguage, {
      maxAge: ONE_YEAR_SECONDS,
      path: "/",
      sameSite: "lax",
      secure,
    });
  }

  if (nextTheme) {
    response.cookies.set(UI_THEME_COOKIE, nextTheme, {
      maxAge: ONE_YEAR_SECONDS,
      path: "/",
      sameSite: "lax",
      secure,
    });
  }

  return response;
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function readLanguage(value: FormDataEntryValue | null): UiLanguage | null {
  return value === "zh" || value === "en" ? value : null;
}

function readTheme(value: FormDataEntryValue | null): UiTheme | null {
  return value === "light" || value === "dark" ? value : null;
}

function safeReturnPath(value: string) {
  if (!value || !value.startsWith("/") || value.startsWith("//")) {
    return "/";
  }
  try {
    const url = new URL(value, "http://local");
    if (url.pathname.startsWith("/api/")) {
      return "/";
    }
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return "/";
  }
}

function safeRefererPath(request: Request) {
  const referer = request.headers.get("referer");
  if (!referer) {
    return null;
  }
  try {
    const refererUrl = new URL(referer);
    const requestHost = request.headers.get("x-forwarded-host")?.trim() || request.headers.get("host")?.trim();
    if (requestHost && refererUrl.host !== requestHost) {
      return null;
    }
    if (refererUrl.pathname.startsWith("/api/")) {
      return null;
    }
    return `${refererUrl.pathname}${refererUrl.search}`;
  } catch {
    return null;
  }
}

function withLocale(path: string, lang: UiLanguage) {
  const url = new URL(path, "http://local");
  const currentPath = url.pathname;
  const localizedPath =
    currentPath === "/"
      ? `/${lang}`
      : /^\/(?:zh|en)(?:\/|$)/.test(currentPath)
        ? currentPath.replace(/^\/(?:zh|en)(?=\/|$)/, `/${lang}`)
        : `/${lang}${currentPath}`;
  return `${localizedPath}${url.search}${url.hash}`;
}
