import {
  AuthProxyError,
  authApiGet,
  authApiPost,
  buildErrorRedirect,
  buildSessionRedirect,
  localizedAdminPath,
  localizedAppPath,
} from "../../../../lib/auth";

type LoginProfile = {
  admin_access_granted?: boolean;
};

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

    const profile = await authApiGet<LoginProfile>("/profile", response.session_token).catch(() => null);
    const fallbackPath = profile?.admin_access_granted
      ? localizedAdminPath(request, "/dashboard")
      : localizedAppPath(request, "/dashboard");

    return buildSessionRedirect(request, next, response.session_token, fallbackPath);
  } catch (error) {
    return buildErrorRedirect(request, "/login", {
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
