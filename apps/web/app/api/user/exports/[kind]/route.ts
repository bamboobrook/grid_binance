import { NextResponse } from "next/server";

import { localizedPublicPath, publicUrl } from "../../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

const EXPORT_PATHS: Record<string, string> = {
  fills: "/exports/fills.csv",
  orders: "/exports/orders.csv",
  payments: "/exports/payments.csv",
  "strategy-stats": "/exports/strategy-stats.csv",
};

export async function GET(
  request: Request,
  context: { params: Promise<{ kind: string }> },
) {
  const { kind } = await context.params;
  const sessionToken = readSessionToken(request);
  const path = EXPORT_PATHS[kind];

  if (!sessionToken) {
    return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?error=session+expired")), { status: 303 });
  }
  if (!path) {
    return new NextResponse("export not found", { status: 404 });
  }

  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return new NextResponse(await response.text(), { status: response.status });
  }

  const body = await response.text();
  return new NextResponse(body, {
    status: 200,
    headers: {
      "content-disposition": `attachment; filename="${kind}.csv"`,
      "content-type": response.headers.get("content-type") ?? "text/csv; charset=utf-8",
    },
  });
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
