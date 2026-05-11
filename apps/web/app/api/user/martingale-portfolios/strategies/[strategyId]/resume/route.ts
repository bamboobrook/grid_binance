import { proxyBacktestRequest } from "../../../../backtest/proxy";

export async function POST(
  request: Request,
  context: { params: Promise<{ strategyId: string }> },
) {
  const { strategyId } = await context.params;
  return proxyBacktestRequest(request, {
    backendPath: `/strategies/${strategyId}/resume`,
  });
}
