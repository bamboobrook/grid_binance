import { NextResponse } from "next/server";
import { localizedPath, localizedPublicPath, publicUrl } from "@/lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const SESSION_TOKEN_COOKIE = "session_token";
const PENDING_TOTP_SECRET_COOKIE = "pending_totp_secret";
const PENDING_TOTP_CODE_COOKIE = "pending_totp_code";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);
  const profile = sessionToken ? await fetchProfile(sessionToken) : null;

  if (!sessionToken || !profile) {
    return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?error=session+expired")), { status: 303 });
  }

  if (intent === "password") {
    return handlePasswordChange(request, sessionToken, readField(formData, "currentPassword"), readField(formData, "password"));
  }

  if (intent === "enable-totp") {
    const result = await authPost(
      "/security/totp/enable",
      sessionToken,
      { email: profile.email },
    );

    if (!result.ok) {
      return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/security?error=${encodeURIComponent(result.error ?? "TOTP enable failed")}`)), { status: 303 });
    }

    const secret = typeof result.data?.secret === "string" ? result.data.secret : "";
    const code = typeof result.data?.code === "string" ? result.data.code : "";
    const response = NextResponse.redirect(publicUrl(request, localizedPath(request, "/app/security?security=totp-enabled")), { status: 303 });
    response.cookies.set(PENDING_TOTP_SECRET_COOKIE, secret, {
      httpOnly: true,
      path: "/",
      sameSite: "lax",
      secure: false,
    });
    response.cookies.set(PENDING_TOTP_CODE_COOKIE, code, {
      httpOnly: true,
      path: "/",
      sameSite: "lax",
      secure: false,
    });
    return response;
  }

  if (intent === "disable-totp") {
    const result = await authPost(
      "/security/totp/disable",
      sessionToken,
      { email: profile.email },
    );

    if (!result.ok) {
      return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/security?error=${encodeURIComponent(result.error ?? "TOTP disable failed")}`)), { status: 303 });
    }

    const response = NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?security=totp-disabled")), { status: 303 });
    response.cookies.delete(SESSION_TOKEN_COOKIE);
    response.cookies.delete(PENDING_TOTP_SECRET_COOKIE);
    response.cookies.delete(PENDING_TOTP_CODE_COOKIE);
    return response;
  }

  return NextResponse.redirect(publicUrl(request, localizedPath(request, "/app/security")), { status: 303 });
}

async function handlePasswordChange(request: Request, sessionToken: string, currentPassword: string, newPassword: string) {
  const result = await authPost(
    "/profile/password/change",
    sessionToken,
    { current_password: currentPassword, new_password: newPassword },
  );

  if (!result.ok) {
    return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/security?error=${encodeURIComponent(result.error ?? "Password change failed")}`)), { status: 303 });
  }

  const response = NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?security=password-updated")), { status: 303 });
  response.cookies.delete(SESSION_TOKEN_COOKIE);
  response.cookies.delete(PENDING_TOTP_SECRET_COOKIE);
  response.cookies.delete(PENDING_TOTP_CODE_COOKIE);
  return response;
}

async function authPost(path: string, sessionToken: string, body: Record<string, string>) {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });

  if (response.ok) {
    return { ok: true as const, data: await response.json() };
  }

  return { ok: false as const, error: await readError(response) };
}

async function fetchProfile(sessionToken: string) {
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: {
      authorization: `Bearer ${sessionToken}`,
    },
    cache: "no-store",
  });

  if (!response.ok) {
    return null;
  }

  const payload = (await response.json()) as { email?: string; totp_enabled?: boolean };
  if (typeof payload.email !== "string") {
    return null;
  }

  return {
    email: payload.email,
    totpEnabled: Boolean(payload.totp_enabled),
  };
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "security request failed";
  } catch {
    return "security request failed";
  }
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
