import { NextRequest, NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: NextRequest) {
  const sessionToken = request.cookies.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const contentType = request.headers.get("content-type") ?? "";
  let payload: Record<string, unknown>;

  if (contentType.includes("application/json")) {
    payload = (await request.json()) as Record<string, unknown>;
  } else {
    const formData = await request.formData();
    const symbol = formData.get("symbol");
    const strategyType = formData.get("strategy_type");
    const startDate = formData.get("start_date");
    const endDate = formData.get("end_date");
    const market = formData.get("market");
    const interval = formData.get("interval");
    const equalMode = formData.get("equal_mode");
    payload = {
      symbol: typeof symbol === "string" ? symbol.trim() : "",
      strategy_type: typeof strategyType === "string" ? strategyType.trim() : "ordinary_grid",
      start_date: typeof startDate === "string" ? startDate.trim() : "",
      end_date: typeof endDate === "string" ? endDate.trim() : "",
      lower_price: Number(formData.get("lower_price") ?? 0),
      upper_price: Number(formData.get("upper_price") ?? 0),
      grid_count: parseInt(String(formData.get("grid_count") ?? "0"), 10),
      investment: Number(formData.get("investment") ?? 0),
      ...(market ? { market: String(market) } : {}),
      ...(interval ? { interval: String(interval) } : {}),
      ...(equalMode ? { equal_mode: String(equalMode) } : {}),
    };
  }

  const apiBase = process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;

  const response = await fetch(`${apiBase}/backtest/run`, {
    method: "POST",
    headers: {
      "authorization": `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  let data: unknown;
  try {
    data = await response.json();
  } catch {
    const text = await response.text();
    data = { error: text || "backtest request failed" };
  }

  return NextResponse.json(data, { status: response.status });
}