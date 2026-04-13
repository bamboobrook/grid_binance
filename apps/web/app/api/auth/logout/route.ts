import { cookies } from "next/headers";
import { NextResponse } from "next/server";

export async function POST(request: Request) {
  const url = new URL(request.url);
  const locale = url.searchParams.get("locale") || "zh";
  const response = NextResponse.redirect(new URL(`/${locale}/login`, request.url));
  (await cookies()).delete("session_token");
  return response;
}
