import { proxyBacktestRequest } from "../../backtest/proxy";

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  return proxyBacktestRequest(request, {
    backendPath: `/martingale-portfolios/${id}`,
  });
}
