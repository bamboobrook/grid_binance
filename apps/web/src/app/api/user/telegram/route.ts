import { NextResponse } from "next/server";

import { updateUserProductState } from "../../../../lib/api/user-product-state";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");

  updateUserProductState(readSessionToken(request), (state) => {
    if (intent === "generate") {
      state.telegram.bindCode = `GB-${String(Date.now()).slice(-4)}`;
      state.telegram.bindCodeIssuedAt = "2026-04-02 10:22";
      state.telegram.state = "code_issued";
      state.flash.telegram = "Bind code issued";
    }

    if (intent === "confirm" && state.telegram.bindCode) {
      state.telegram.state = "bound";
      state.telegram.boundAt = "2026-04-02 10:24";
      state.flash.telegram = "Telegram bound";
    }
  });

  return NextResponse.redirect(new URL("/app/telegram", request.url), { status: 303 });
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
