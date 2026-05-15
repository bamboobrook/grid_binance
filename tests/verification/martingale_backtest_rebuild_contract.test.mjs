import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("martingale wizard defaults to automatic search and current previous-month range", () => {
  const source = read("apps/web/components/backtest/backtest-wizard.tsx");
  assert.match(source, /开始自动搜索 Top 10|Start automatic Top 10 search/);
  assert.match(source, /自动时间范围|Automatic time range/i);
  assert.match(source, /2023-01-01/);
  assert.match(source, /上个月月底|previous month end/i);
  assert.match(source, /auto_since_2023_to_last_month_end|auto_previous_month_end/);
  assert.match(source, /per_symbol_top_n:\s*10/);
  assert.match(source, /portfolio_top_n:\s*3/);
  assert.match(source, /market:\s*["']futures["']|market:\s*["']usd_m_futures["']/);
  assert.doesNotMatch(source, /<option value="spot">|Spot<\/option>/);
  assert.match(source, /risk_profile/);
  assert.match(source, /高级参数搜索范围|Advanced parameter search space/);
  assert.doesNotMatch(source, /默认.*加仓间距|Default.*spacing/i);
});

test("backtest console exposes progress, per-symbol Top 10, portfolio Top 3, charts, and basket publish", () => {
  const consoleSource = read("apps/web/components/backtest/backtest-console.tsx");
  const tableSource = read("apps/web/components/backtest/backtest-result-table.tsx");
  const chartSource = read("apps/web/components/backtest/backtest-charts.tsx");
  const basketSource = read("apps/web/components/backtest/portfolio-candidate-review.tsx");
  const resultSource = `${consoleSource}\n${tableSource}`;

  assert.match(resultSource, /排队中|运行中|已完成|失败|Queued|Running|Succeeded|Failed/);
  assert.match(resultSource, /已评估候选|evaluated candidates/i);
  assert.match(resultSource, /已完成币种|completed symbols/i);
  assert.match(resultSource, /每个币种 Top 10|Per-symbol Top 10/i);
  assert.match(resultSource, /组合 Top 3|Portfolio Top 3/i);
  assert.match(resultSource, /年化收益|Annualized/i);
  assert.match(resultSource, /收益回撤比|Return\/DD/i);
  assert.match(resultSource, /0–100|0-100|百分制/i);
  assert.match(resultSource, /加入组合|Add to basket/);
  assert.match(resultSource, /参数排名|Parameter rank/i);
  assert.match(resultSource, /parameter_rank_for_symbol/);
  assert.match(chartSource, /资金曲线|Equity curve/i);
  assert.match(chartSource, /回撤曲线|Drawdown curve/i);
  assert.match(chartSource, /图表数据缺失|chart data is missing/i);
  assert.match(basketSource, /组合篮子|Portfolio basket/i);
  assert.match(basketSource, /权重合计|Weight total/i);
  assert.match(basketSource, /批量发布实盘组合|Batch publish live portfolio/);
});

test("frontend has proxy route for batch portfolio publish", () => {
  const route = read("apps/web/app/api/user/backtest/portfolios/publish/route.ts");
  assert.match(route, /backendPath:\s*["']\/backtest\/portfolios\/publish["']/);
  assert.match(route, /POST/);
});
