import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("strategy runtime recovery surfaces stop-state sync, list summaries, and non-jumping row actions", () => {
  const engineSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/trading-engine/src/main.rs");
  const listPageSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/page.tsx");
  const tableSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-inventory-table.tsx");
  const routeSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/user/strategies/[id]/route.ts");

  assert.match(engineSource, /StrategyStatus::Stopping/, "trading engine live-sync gate should include stopping strategies so stop requests can reconcile close orders");
  assert.match(listPageSource, /fetchAnalytics\(/, "strategy list should fetch analytics so key metrics are visible without opening details");
  assert.match(tableSource, /Grid Count|网格总数/, "strategy list should expose grid-count summary");
  assert.match(tableSource, /Fill Count|成交数量/, "strategy list should expose fill-count summary");
  assert.match(tableSource, /Average Cost|平均持仓成本/, "strategy list should expose average holding cost");
  assert.match(tableSource, /Trade Count|交易次数/, "strategy list should expose trade-count summary");
  assert.doesNotMatch(listPageSource, /tradeCount:\s*summary\?\.order_count/, "strategy list trade count should not be derived from total order rows");
  assert.match(listPageSource, /tradeCount:\s*summary\?\.fill_count/, "strategy list trade count should follow actual fill count by default");
  assert.match(tableSource, /Grid PnL|网格盈亏/, "strategy list should expose grid PnL");
  assert.match(tableSource, /Overall PnL|总体盈亏/, "strategy list should expose overall PnL");
  assert.match(routeSource, /returnTo/, "row lifecycle route should accept an explicit list redirect hint");
  assert.match(routeSource, /\/strategies\?notice=strategy-started/, "row lifecycle route should be able to return to the list after start");
  assert.match(routeSource, /\/strategies\?notice=strategy-stopped/, "row lifecycle route should be able to return to the list after stop");
});
