import type { NextRequest } from "next/server";
import { NextResponse } from "next/server";

const ADMIN_SESSION_COOKIE = "admin_session";
const ADMIN_SESSION_VALUE = "active";

export function proxy(request: NextRequest) {
  const adminSession = request.cookies.get(ADMIN_SESSION_COOKIE)?.value;

  if (adminSession === ADMIN_SESSION_VALUE) {
    return NextResponse.next();
  }

  const loginUrl = new URL("/login", request.url);
  loginUrl.searchParams.set(
    "next",
    `${request.nextUrl.pathname}${request.nextUrl.search}`,
  );

  return NextResponse.redirect(loginUrl);
}

export const config = {
  matcher: ["/admin/:path*"],
};
