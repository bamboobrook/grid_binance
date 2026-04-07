import { NextResponse } from "next/server";

import { AuthProxyError, authApiPost } from "../../../../lib/auth";

export async function POST(request: Request) {
  const secureCookie = process.env.NODE_ENV === "production";
  const formData = await request.formData();
  const email = readField(formData, "email");
  const password = readField(formData, "password");
  const next = readField(formData, "next");

  try {
    const registerResponse = await authApiPost<{ user_id: number; code_delivery: string; verification_code?: string }>("/auth/register", {
      email,
      password,
    });

    const url = new URL("/verify-email", request.url);
    url.searchParams.set("email", email);
    if (next) {
      url.searchParams.set("next", next);
    }
    url.searchParams.set("notice", "registration-created");

    const response = NextResponse.redirect(url, { status: 303 });
    if (registerResponse.verification_code) {
      response.cookies.set("pending_verify_code", registerResponse.verification_code, {
        httpOnly: true,
        path: "/",
        sameSite: "lax",
        secure: secureCookie,
      });
    }
    return response;
  } catch (error) {
    const url = new URL("/register", request.url);
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

  return "auth request failed";
}
