import { cookies } from "next/headers";
import { NextResponse } from "next/server";
import { publicUrl } from "@/lib/auth";

export async function POST(request: Request) {
  const url = new URL(request.url);
  const locale = url.searchParams.get("locale") || "zh";
  const response = NextResponse.redirect(publicUrl(request, `/${locale}`), 303);
  (await cookies()).delete("session_token");
  return response;
}
