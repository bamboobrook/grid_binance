import { proxyBacktestRequest } from "../../proxy";

export async function POST(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/portfolios/publish",
  });
}
