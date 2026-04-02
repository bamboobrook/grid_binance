import { NextResponse } from "next/server";

import { updateUserProductState } from "../../../../lib/api/user-product-state";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const SESSION_TOKEN_COOKIE = "session_token";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);
  const profile = sessionToken ? await fetchProfile(sessionToken) : null;

  if (!sessionToken || !profile) {
    return NextResponse.redirect(new URL("/login?error=session+expired", request.url), { status: 303 });
  }

  if (intent === "password") {
    const currentPassword = readField(formData, "currentPassword");
    const newPassword = readField(formData, "password");
    const result = await authPost(
      "/profile/password/change",
      sessionToken,
      { current_password: currentPassword, new_password: newPassword },
    );

    if (!result.ok) {
      return NextResponse.redirect(new URL(`/app/security?error=${encodeURIComponent(result.error ?? "Password change failed")}`, request.url), { status: 303 });
    }

    updateUserProductState(sessionToken, (state) => {
      state.security.passwordChangedAt = "2026-04-02 10:12";
      state.flash.security = null;
    });

    const response = NextResponse.redirect(new URL("/login?security=password-updated", request.url), { status: 303 });
    response.cookies.delete(SESSION_TOKEN_COOKIE);
    return response;
  }

  if (intent === "enable-totp") {
    const result = await authPost(
      "/security/totp/enable",
      sessionToken,
      { email: profile.email },
    );

    if (!result.ok) {
      return NextResponse.redirect(new URL(`/app/security?error=${encodeURIComponent(result.error ?? "TOTP enable failed")}`, request.url), { status: 303 });
    }

    updateUserProductState(sessionToken, (state) => {
      state.security.totpEnabled = true;
      state.flash.security = "TOTP enabled";
    });

    return NextResponse.redirect(new URL("/app/security", request.url), { status: 303 });
  }

  if (intent === "disable-totp") {
    const result = await authPost(
      "/security/totp/disable",
      sessionToken,
      { email: profile.email },
    );

    if (!result.ok) {
      return NextResponse.redirect(new URL(`/app/security?error=${encodeURIComponent(result.error ?? "TOTP disable failed")}`, request.url), { status: 303 });
    }

    updateUserProductState(sessionToken, (state) => {
      state.security.totpEnabled = false;
      state.flash.security = null;
    });

    const response = NextResponse.redirect(new URL("/login?security=totp-disabled", request.url), { status: 303 });
    response.cookies.delete(SESSION_TOKEN_COOKIE);
    return response;
  }

  return NextResponse.redirect(new URL("/app/security", request.url), { status: 303 });
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
