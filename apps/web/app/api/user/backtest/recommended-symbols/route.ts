import { proxyBacktestRequest } from "../proxy";

export async function GET(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/recommended-symbols",
  });
}
