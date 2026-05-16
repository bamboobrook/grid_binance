import { proxyBacktestRequest } from "../backtest/proxy";

export async function GET(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/martingale-portfolios",
  });
}
