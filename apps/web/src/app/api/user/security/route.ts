import { NextResponse } from "next/server";

import { updateUserProductState } from "../../../../lib/api/user-product-state";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");

  updateUserProductState(readSessionToken(request), (state) => {
    if (intent === "password") {
      state.security.passwordChangedAt = "2026-04-02 10:12";
      state.flash.security = "Password updated";
    }

    if (intent === "enable-totp") {
      state.security.totpEnabled = true;
      state.flash.security = "TOTP enabled";
    }

    if (intent === "disable-totp") {
      state.security.totpEnabled = false;
      state.flash.security = "TOTP disabled";
    }

    if (intent === "revoke-sessions") {
      state.security.sessionsRevokedAt = "2026-04-02 10:15";
      state.flash.security = "Other sessions revoked";
    }
  });

  return NextResponse.redirect(new URL("/app/security", request.url), { status: 303 });
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
