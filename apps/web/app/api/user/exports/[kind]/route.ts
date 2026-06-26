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
    if (process.env.NEXT_PUBLIC_UI_PREVIEW === "1") {
      const previewCsv = previewExport(kind);
      if (previewCsv) {
        return csvResponse(kind, previewCsv);
      }
    }
    return new NextResponse(await response.text(), { status: response.status });
  }

  const body = await response.text();
  return csvResponse(kind, body, response.headers.get("content-type"));
}

function csvResponse(kind: string, body: string, contentType?: string | null) {
  return new NextResponse(body, {
    status: 200,
    headers: {
      "content-disposition": `attachment; filename="${kind}.csv"`,
      "content-type": contentType ?? "text/csv; charset=utf-8",
    },
  });
}

function previewExport(kind: string) {
  if (kind === "orders") {
    return [
      "order_id,strategy,symbol,side,quantity,price,status",
      "GB-BTC-1208,BTC steady grid,BTCUSDT,Buy,0.006,86420.00,Working",
      "GB-BTC-1214,BTC steady grid,BTCUSDT,Sell,0.004,88160.00,Placed",
      "GB-ETH-0603,ETH small futures test,ETHUSDT,Buy,0.18,3412.50,Working",
      "MT-SOL-0327,SOL DCA watch bot,SOLUSDT,Sell,12.5,151.80,Canceled",
    ].join("\n");
  }
  if (kind === "fills") {
    return [
      "fill_id,event,symbol,quantity,price,pnl",
      "preview-fill-btc-1,Grid sell,BTCUSDT,0.005,87240.00,+12.48 USDT",
      "preview-fill-eth-1,DCA buy,ETHUSDT,0.22,3388.20,",
      "preview-fill-sol-1,DCA reduce,SOLUSDT,8.4,148.60,+7.31 USDT",
    ].join("\n");
  }
  return null;
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
