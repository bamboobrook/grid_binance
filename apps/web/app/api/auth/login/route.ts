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
  const totpCode = readField(formData, "totpCode");
  const next = readField(formData, "next");

  try {
    const response = await authApiPost<{ session_token: string }>("/auth/login", {
      email,
      password,
      totp_code: totpCode || null,
    });

    return buildSessionRedirect(request.url, next, response.session_token);
  } catch (error) {
    return buildErrorRedirect(request.url, "/login", {
      email,
      next,
      error: errorMessage(error),
      extra: loginExtras(error),
    });
  }
}

function readField(formData: FormData, field: string) {
  const value = formData.get(field);
  return typeof value === "string" ? value.trim() : "";
}

function needsTotp(error: unknown) {
  return error instanceof AuthProxyError && /totp/i.test(error.message);
}

function errorMessage(error: unknown) {
  if (error instanceof AuthProxyError) {
    return error.message;
  }

  return "auth request failed";
}

function loginExtras(error: unknown): Record<string, string> | undefined {
  if (error instanceof AuthProxyError && /admin totp setup required/i.test(error.message)) {
    return { adminBootstrap: "1" };
  }
  if (needsTotp(error)) {
    return { totp: "1" };
  }
  return undefined;
}
