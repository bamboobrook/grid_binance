import { NextResponse } from "next/server";

export async function GET(request: Request) {
  const locale = request.headers.get("cookie")?.includes("ui_lang=en") ? "en" : "zh";
  const url = new URL(`/${locale}/login`, request.url);
  url.searchParams.set("next", `/${locale}/admin/dashboard`);
  return NextResponse.redirect(url, { status: 307 });
}
