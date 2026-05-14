import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vm from "node:vm";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");
const require = createRequire(import.meta.url);
const ts = require("../../node_modules/.pnpm/typescript@5.9.3/node_modules/typescript");

function loadWizardPayloadHelpers() {
  const source = read("apps/web/components/backtest/backtest-wizard.tsx");
  const helperStart = source.indexOf("const DEFAULT_SYMBOLS");
  const helperEnd = source.indexOf("export function BacktestWizard");
  const payloadStart = source.indexOf("export function parseSymbolList");
  const payloadSource = source.slice(helperStart, helperEnd) + source.slice(payloadStart);
  const withoutJsx = payloadSource.replace(/function AutomaticSearchPanel[\s\S]*$/, "");
  const compiled = ts.transpileModule(withoutJsx, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const context = { exports: {}, module: { exports: {} } };
  vm.runInNewContext(compiled, context);
  return context.exports;
}

const baseForm = {
  symbolPoolMode: "whitelist",
  whitelist: "BTCUSDT, ETHUSDT",
  blacklist: "",
  searchMode: "intelligent",
  parameterPreset: "balanced",
  randomSeed: "20260509",
  candidateBudget: "160",
  intelligentRounds: "4",
  topN: "20",
  market: "usd_m_futures",
  directionMode: "long_and_short",
  hedgeModeRequired: true,
  marginMode: "isolated",
  minLeverage: "2",
  maxLeverage: "10",
  initialOrderUsdt: "10",
  spacingPct: "1",
  orderMultiplier: "2",
  maxLegs: "6",
  takeProfitPct: "1",
  trailingPct: "0.4",
  stopLossMode: "portfolio_drawdown",
  timeMode: "manual",
  trainStart: "2023-01-01",
  trainEnd: "2024-12-31",
  validateStart: "2025-01-01",
  validateEnd: "2025-03-31",
  testStart: "2025-04-01",
  testEnd: "2025-06-30",
  interval: "1m",
  maxDrawdownPct: "25",
  maxStopLossCount: "3",
  portfolioStopLossPct: "18",
  perStrategyStopLossPct: "8",
};

test("martingale wizard defaults to automatic search and current previous-month range", () => {
  const source = read("apps/web/components/backtest/backtest-wizard.tsx");
  const wizardSource = source;
  assert.match(source, /开始自动搜索 Top 10|Start automatic Top 10 search/);
  assert.match(source, /自动时间范围|Automatic time range/i);
  assert.match(source, /2023-01-01/);
  assert.match(source, /上个月月底|previous month end/i);
  assert.match(source, /auto_previous_month_end/);
  assert.match(source, /per_symbol_top_n:\s*10/);
  assert.match(source, /risk_profile/);
  assert.match(source, /高级参数搜索范围|Advanced parameter search space/);
  assert.doesNotMatch(source, /默认.*加仓间距|Default.*spacing/i);
  assert.match(wizardSource, /conservative[^\n]+20/);
  assert.match(wizardSource, /balanced[^\n]+25/);
  assert.match(wizardSource, /aggressive[^\n]+30/);
  assert.match(wizardSource, /manualDrawdownOverride/);
  assert.match(wizardSource, /dynamic_allocation_enabled/);
  assert.match(wizardSource, /per_symbol_top_n:\s*10/);
  assert.match(wizardSource, /portfolio_top_n:\s*10/);
});

test("martingale wizard payload preserves dynamic search and manual risk overrides", () => {
  const { buildWizardPayload } = loadWizardPayloadHelpers();
  assert.equal(typeof buildWizardPayload, "function");

  const payload = buildWizardPayload({
    ...baseForm,
    parameterPreset: "balanced",
    maxDrawdownPct: "17.5",
    maxStopLossCount: "9",
  });

  assert.equal(payload.per_symbol_top_n, 10);
  assert.equal(payload.portfolio_top_n, 10);
  assert.equal(payload.dynamic_allocation_enabled, true);
  assert.equal(payload.scoring.max_drawdown_pct, 17.5);
  assert.equal(payload.scoring.max_stop_loss_count, 9);
});

test("martingale wizard risk defaults map 20/25/30 while allowing manual drawdown override", () => {
  const { buildWizardPayload } = loadWizardPayloadHelpers();

  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "conservative", maxDrawdownPct: "20" }).scoring.max_drawdown_pct, 20);
  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "balanced", maxDrawdownPct: "25" }).scoring.max_drawdown_pct, 25);
  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "aggressive", maxDrawdownPct: "30" }).scoring.max_drawdown_pct, 30);
  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "aggressive", maxDrawdownPct: "12.5" }).scoring.max_drawdown_pct, 12.5);

  const wizardSource = read("apps/web/components/backtest/backtest-wizard.tsx");
  assert.match(wizardSource, /manualDrawdownOverride/);
});

test("backtest console exposes progress, grouped top ten, charts, and basket publish", () => {
  const consoleSource = read("apps/web/components/backtest/backtest-console.tsx");
  const tableSource = read("apps/web/components/backtest/backtest-result-table.tsx");
  const chartSource = read("apps/web/components/backtest/backtest-charts.tsx");
  const basketSource = read("apps/web/components/backtest/portfolio-candidate-review.tsx");
  const resultSource = `${consoleSource}\n${tableSource}`;

  assert.match(resultSource, /排队中|运行中|已完成|失败|Queued|Running|Succeeded|Failed/);
  assert.match(resultSource, /已评估候选|evaluated candidates/i);
  assert.match(resultSource, /已完成币种|completed symbols/i);
  assert.match(resultSource, /每个币种 Top 10|Per-symbol Top 10/i);
  assert.match(resultSource, /杠杆|Leverage/i);
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
