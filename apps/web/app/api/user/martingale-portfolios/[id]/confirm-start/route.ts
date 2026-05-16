import { proxyBacktestRequest } from "../../../backtest/proxy";

export async function POST(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params;
  return proxyBacktestRequest(request, {
    backendPath: `/backtest/portfolios/${id}/confirm-start`,
  });
}
