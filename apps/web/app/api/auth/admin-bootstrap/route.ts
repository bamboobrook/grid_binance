import { NextResponse } from "next/server";

import {
  AuthProxyError,
  authApiPost,
  localizedPublicPath,
  publicUrl,
  shouldUseSecureCookie,
} from "../../../../lib/auth";


const PENDING_ADMIN_TOTP_SECRET_COOKIE = "pending_admin_totp_secret";
const PENDING_ADMIN_TOTP_CODE_COOKIE = "pending_admin_totp_code";
const PENDING_ADMIN_TOTP_EMAIL_COOKIE = "pending_admin_totp_email";

export async function POST(request: Request) {
  const formData = await request.formData();
  const email = readField(formData, "email");
  const password = readField(formData, "password");
  const secureCookie = shouldUseSecureCookie(request);

  try {
    const response = await authApiPost<{ secret: string; code: string }>("/auth/admin-bootstrap", {
      email,
      password,
    });
    const url = publicUrl(request, localizedPublicPath(request, "/admin-bootstrap"));
    url.searchParams.set("setup", "ready");
    url.searchParams.set("email", email);
    const redirect = NextResponse.redirect(url, { status: 303 });
    redirect.cookies.set(PENDING_ADMIN_TOTP_SECRET_COOKIE, response.secret, {
      httpOnly: true,
      path: "/",
      sameSite: "lax",
      secure: secureCookie,
    });
    redirect.cookies.set(PENDING_ADMIN_TOTP_CODE_COOKIE, response.code, {
      httpOnly: true,
      path: "/",
      sameSite: "lax",
      secure: secureCookie,
    });
    redirect.cookies.set(PENDING_ADMIN_TOTP_EMAIL_COOKIE, email, {
      httpOnly: true,
      path: "/",
      sameSite: "lax",
      secure: secureCookie,
    });
    return redirect;
  } catch (error) {
    const message = error instanceof AuthProxyError ? error.message : "admin bootstrap request failed";
    const url = publicUrl(request, localizedPublicPath(request, "/admin-bootstrap"));
    url.searchParams.set("error", message);
    url.searchParams.set("email", email);
    return NextResponse.redirect(url, { status: 303 });
  }
}

function readField(formData: FormData, field: string) {
  const value = formData.get(field);
  return typeof value === "string" ? value.trim() : "";
}
