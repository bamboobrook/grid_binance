import { NextResponse } from "next/server";

import { AuthProxyError, authApiPost } from "../../../../lib/auth";

export async function POST(request: Request) {
  const secureCookie = process.env.NODE_ENV === "production";
  const formData = await request.formData();
  const intent = readField(formData, "intent") || "request";
  const email = readField(formData, "email");

  if (intent === "request") {
    return requestReset(request, email, secureCookie);
  }

  return confirmReset(
    request,
    email,
    readField(formData, "code"),
    readField(formData, "password"),
  );
}

async function requestReset(request: Request, email: string, secureCookie: boolean) {
  try {
    const responseBody = await authApiPost<{ code_delivery: string; reset_code?: string }>("/auth/password-reset/request", { email });
    const url = new URL("/password-reset", request.url);
    url.searchParams.set("email", email);
    url.searchParams.set("step", "confirm");
    url.searchParams.set("notice", "reset-code-issued");
    const response = NextResponse.redirect(url, { status: 303 });
    if (responseBody.reset_code) {
      response.cookies.set("pending_reset_code", responseBody.reset_code, {
        httpOnly: true,
        path: "/",
        sameSite: "lax",
        secure: secureCookie,
      });
    }
    return response;
  } catch (error) {
    const url = new URL("/password-reset", request.url);
    url.searchParams.set("email", email);
    url.searchParams.set("error", errorMessage(error));
    return NextResponse.redirect(url, { status: 303 });
  }
}

async function confirmReset(request: Request, email: string, code: string, password: string) {
  try {
    await authApiPost("/auth/password-reset/confirm", {
      email,
      code,
      new_password: password,
    });
    const url = new URL("/login", request.url);
    url.searchParams.set("email", email);
    url.searchParams.set("notice", "password-reset-complete");
    return NextResponse.redirect(url, { status: 303 });
  } catch (error) {
    const url = new URL("/password-reset", request.url);
    url.searchParams.set("email", email);
    url.searchParams.set("step", "confirm");
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

  return "password reset request failed";
}
