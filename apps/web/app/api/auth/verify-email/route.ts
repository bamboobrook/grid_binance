import { NextResponse } from "next/server";

import { AuthProxyError, authApiPost } from "../../../../lib/auth";
import { localizedPath, localizedPublicPath, publicUrl } from "@/lib/auth";

const PENDING_VERIFY_CODE_COOKIE = "pending_verify_code";

export async function POST(request: Request) {
  const formData = await request.formData();
  const email = readField(formData, "email");
  const code = readField(formData, "code");
  const next = readField(formData, "next");

  try {
    await authApiPost("/auth/verify-email", { email, code });
    const url = publicUrl(request, localizedPublicPath(request, "/login"));
    url.searchParams.set("email", email);
    if (next) {
      url.searchParams.set("next", next);
    }
    url.searchParams.set("notice", "email-verified");
    const response = NextResponse.redirect(url, { status: 303 });
    response.cookies.delete(PENDING_VERIFY_CODE_COOKIE);
    return response;
  } catch (error) {
    const url = publicUrl(request, localizedPublicPath(request, "/verify-email"));
    url.searchParams.set("email", email);
    if (next) {
      url.searchParams.set("next", next);
    }
    url.searchParams.set("error", errorMessage(error));
    return NextResponse.redirect(url, { status: 303 });
  }
}

function readField(formData: FormData, field: string) {
  const value = formData.get(field);
  return typeof value === "string" ? value.trim() : "";
}

function errorMessage(error: unknown) {
  if (error instanceof AuthProxyError) {
    return error.message;
  }

  return "verify email request failed";
}
