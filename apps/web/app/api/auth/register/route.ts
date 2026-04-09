import { NextResponse } from "next/server";

import { AuthProxyError, authApiPost, localizedPublicPath, publicUrl } from "../../../../lib/auth";

export async function POST(request: Request) {
  const formData = await request.formData();
  const email = readField(formData, "email");
  const password = readField(formData, "password");
  const next = readField(formData, "next");

  try {
    await authApiPost<{ user_id: number; code_delivery: string; verification_code?: string }>("/auth/register", {
      email,
      password,
    });

    const url = publicUrl(request, localizedPublicPath(request, "/login"));
    url.searchParams.set("email", email);
    if (next) {
      url.searchParams.set("next", next);
    }
    url.searchParams.set("notice", "registration-created");

    return NextResponse.redirect(url, { status: 303 });
  } catch (error) {
    const url = publicUrl(request, localizedPublicPath(request, "/register"));
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

