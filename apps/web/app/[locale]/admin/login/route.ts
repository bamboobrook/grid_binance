import { NextResponse } from "next/server";

import { publicUrl, safeRedirectTarget } from "@/lib/auth";

export async function GET(request: Request, props: { params: Promise<{ locale: string }> }) {
  const params = await props.params;
  const requestUrl = new URL(request.url);
  const loginUrl = publicUrl(request, `/${params.locale}/login`);
  const next = safeRedirectTarget(
    requestUrl.searchParams.get("next"),
    `/${params.locale}/admin/dashboard`,
  );

  loginUrl.searchParams.set("next", next);

  for (const key of ["email", "error", "notice", "security", "totp", "adminBootstrap"]) {
    const value = requestUrl.searchParams.get(key);
    if (value) {
      loginUrl.searchParams.set(key, value);
    }
  }

  return NextResponse.redirect(loginUrl, { status: 303 });
}
