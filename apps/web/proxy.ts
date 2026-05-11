import createMiddleware from "next-intl/middleware";
import { NextResponse, type NextRequest } from "next/server";
import { publicUrl } from "@/lib/auth";

const locales = ["en", "zh"] as const;
const defaultLocale = "zh";
const intlMiddleware = createMiddleware({
  locales,
  defaultLocale,
  localePrefix: "always",
});

export default function proxy(request: NextRequest) {
  const locale = localeFromPath(request.nextUrl.pathname);
  if (!locale) {
    return intlMiddleware(request);
  }

  const pathname = stripLocale(request.nextUrl.pathname);
  const sessionToken = request.cookies.get("session_token")?.value ?? "";
  const isAdminLoginEntry = pathname === "/admin/login" || pathname.startsWith("/admin/login/");

  if (!sessionToken && (pathname.startsWith("/app") || (pathname.startsWith("/admin") && !isAdminLoginEntry))) {
    const url = publicUrl(request, `/${locale}/login`);
    url.searchParams.set("next", request.nextUrl.pathname);
    return NextResponse.redirect(url);
  }

  return intlMiddleware(request);
}

function localeFromPath(pathname: string) {
  const match = pathname.match(/^\/(en|zh)(?=\/|$)/);
  return match?.[1] ?? null;
}

function stripLocale(pathname: string) {
  return pathname.replace(/^\/(en|zh)(?=\/|$)/, "") || "/";
}

export const config = {
  matcher: ["/", "/(zh|en)/:path*", "/((?!api|_next|_static|_vercel|[\\w-]+\\.\\w+).*)"],
};
