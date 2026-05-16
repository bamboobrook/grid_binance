import { proxyBacktestRequest } from "../proxy";

export async function GET(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/tasks",
  });
}

export async function POST(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/tasks",
  });
}
