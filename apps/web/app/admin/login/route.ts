import { NextResponse } from "next/server";
import { publicUrl } from "@/lib/auth";

export async function GET(request: Request) {
  const locale = request.headers.get("cookie")?.includes("ui_lang=en") ? "en" : "zh";
  const url = publicUrl(request, `/${locale}/login`);
  url.searchParams.set("next", `/${locale}/admin/dashboard`);
  return NextResponse.redirect(url, { status: 307 });
}
