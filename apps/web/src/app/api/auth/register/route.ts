import {
  AuthProxyError,
  authApiPost,
  buildErrorRedirect,
  buildSessionRedirect,
  type RegisterResponse,
} from "../../../../lib/auth";

export async function POST(request: Request) {
  const formData = await request.formData();
  const email = readField(formData, "email");
  const password = readField(formData, "password");
  const next = readField(formData, "next");

  try {
    const registerResponse = await authApiPost<RegisterResponse>("/auth/register", {
      email,
      password,
    });
    await authApiPost("/auth/verify-email", {
      email,
      code: registerResponse.verification_code,
    });
    const loginResponse = await authApiPost<{ session_token: string }>("/auth/login", {
      email,
      password,
      totp_code: null,
    });

    return buildSessionRedirect(request.url, next, loginResponse.session_token);
  } catch (error) {
    return buildErrorRedirect(request.url, "/register", {
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
