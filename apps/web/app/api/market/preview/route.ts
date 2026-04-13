import { NextResponse } from "next/server";

type PreviewCandle = {
  close: number;
  close_time: number;
  high: number;
  low: number;
  open: number;
  open_time: number;
};

export async function GET(request: Request) {
  const url = new URL(request.url);
  const symbol = url.searchParams.get("symbol")?.trim().toUpperCase() ?? "";
  const marketType = normalizeMarketType(url.searchParams.get("marketType"));

  if (symbol === "") {
    return NextResponse.json({ error: "symbol is required" }, { status: 400 });
  }

  try {
    const [candlesResponse, priceResponse] = await Promise.all([
      fetch(binanceKlinesUrl(symbol, marketType), { cache: "no-store" }),
      fetch(binanceTickerUrl(symbol, marketType), { cache: "no-store" }),
    ]);

    if (!candlesResponse.ok) {
      return NextResponse.json({ error: "market preview unavailable" }, { status: 502 });
    }

    const candlePayload = (await candlesResponse.json()) as unknown;
    const candles = Array.isArray(candlePayload)
      ? candlePayload
          .map(parseBinanceKline)
          .filter((item): item is PreviewCandle => item !== null)
      : [];

    let latestPrice: string | null = null;
    if (priceResponse.ok) {
      const pricePayload = (await priceResponse.json()) as { price?: string };
      latestPrice = typeof pricePayload.price === "string" ? pricePayload.price : null;
    }

    return NextResponse.json({
      candles,
      latest_price: latestPrice,
      market_type: marketType,
      symbol,
    });
  } catch {
    return NextResponse.json({ error: "market preview unavailable" }, { status: 502 });
  }
}

function parseBinanceKline(input: unknown): PreviewCandle | null {
  if (!Array.isArray(input) || input.length < 7) {
    return null;
  }

  const openTime = Number(input[0]);
  const open = Number(input[1]);
  const high = Number(input[2]);
  const low = Number(input[3]);
  const close = Number(input[4]);
  const closeTime = Number(input[6]);

  if (
    !Number.isFinite(openTime)
    || !Number.isFinite(closeTime)
    || !Number.isFinite(open)
    || !Number.isFinite(high)
    || !Number.isFinite(low)
    || !Number.isFinite(close)
  ) {
    return null;
  }

  return {
    close,
    close_time: closeTime,
    high,
    low,
    open,
    open_time: openTime,
  };
}

function normalizeMarketType(value: string | null) {
  switch (value) {
    case "usd-m":
    case "coin-m":
    case "spot":
      return value;
    default:
      return "spot";
  }
}

function binanceKlinesUrl(symbol: string, marketType: "spot" | "usd-m" | "coin-m") {
  const encodedSymbol = encodeURIComponent(symbol);
  switch (marketType) {
    case "usd-m":
      return `https://fapi.binance.com/fapi/v1/klines?symbol=${encodedSymbol}&interval=15m&limit=48`;
    case "coin-m":
      return `https://dapi.binance.com/dapi/v1/klines?symbol=${encodedSymbol}&interval=15m&limit=48`;
    default:
      return `https://api.binance.com/api/v3/klines?symbol=${encodedSymbol}&interval=15m&limit=48`;
  }
}

function binanceTickerUrl(symbol: string, marketType: "spot" | "usd-m" | "coin-m") {
  const encodedSymbol = encodeURIComponent(symbol);
  switch (marketType) {
    case "usd-m":
      return `https://fapi.binance.com/fapi/v1/ticker/price?symbol=${encodedSymbol}`;
    case "coin-m":
      return `https://dapi.binance.com/dapi/v1/ticker/price?symbol=${encodedSymbol}`;
    default:
      return `https://api.binance.com/api/v3/ticker/price?symbol=${encodedSymbol}`;
  }
}
