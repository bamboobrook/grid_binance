import {
  AuthProxyError,
  authApiPost,
  buildErrorRedirect,
  buildSessionRedirect,
} from "../../../../lib/auth";

export async function POST(request: Request) {
  const formData = await request.formData();
  const email = readField(formData, "email");
  const password = readField(formData, "password");
  const next = readField(formData, "next");

  try {
    const response = await authApiPost<{ session_token: string }>("/auth/login", {
      email,
      password,
      totp_code: null,
    });

    return buildSessionRedirect(request.url, next, response.session_token);
  } catch (error) {
    return buildErrorRedirect(request.url, "/login", {
      email,
      next,
      error: errorMessage(error),
    });
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
