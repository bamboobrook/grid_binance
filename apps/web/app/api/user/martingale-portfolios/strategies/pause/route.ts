import { proxyBacktestRequest } from "../../../backtest/proxy";

export async function POST(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/strategies/batch/pause",
  });
}
