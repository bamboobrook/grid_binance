import { NextResponse } from "next/server";

export function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

export function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

export function redirectTo(request: Request, path: string) {
  return NextResponse.redirect(new URL(path, request.url), { status: 303 });
}
