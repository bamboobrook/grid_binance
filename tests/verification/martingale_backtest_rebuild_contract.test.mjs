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

function loadBacktestConsoleHelpers() {
  const source = read("apps/web/components/backtest/backtest-console.tsx");
  const helperStart = source.indexOf("function normalizeCandidate");
  const helperEnd = source.indexOf("function formatDate");
  assert.notEqual(helperStart, -1);
  assert.notEqual(helperEnd, -1);
  const helperSource = `type UiLanguage = "zh" | "en";\n${source.slice(helperStart, helperEnd)}\nexports.normalizeCandidate = normalizeCandidate;`;
  const compiled = ts.transpileModule(helperSource, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const pickText = (lang, zh, en) => (lang === "zh" ? zh : en);
  const context = {
    exports: {},
    module: { exports: {} },
    require: (id) => (id === "react/jsx-runtime" ? { jsx: () => null, jsxs: () => null, Fragment: Symbol("Fragment") } : require(id)),
    pickText,
    Number,
    Math,
    Array,
    Object,
    String,
  };
  vm.runInNewContext(compiled, context);
  return context.exports;
}

function loadBacktestChartHelpers() {
  const source = read("apps/web/components/backtest/backtest-charts.tsx");
  const helperStart = source.indexOf("function fmtNum");
  const helperEnd = source.indexOf("/* ------------------------------------------------------------------ */\n/*  Stress window badges");
  assert.notEqual(helperStart, -1);
  assert.notEqual(helperEnd, -1);
  const helperSource = `${source.slice(helperStart, helperEnd)}\nexports.normalizeCostSummary = normalizeCostSummary;\nexports.formatCost = formatCost;\nexports.formatCount = formatCount;\nexports.formatHours = formatHours;`;
  const compiled = ts.transpileModule(helperSource, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const context = {
    exports: {},
    module: { exports: {} },
    require: (id) => (id === "react/jsx-runtime" ? { jsx: () => null, jsxs: () => null, Fragment: Symbol("Fragment") } : require(id)),
    Number,
    Array,
    Object,
    String,
  };
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

test("martingale wizard payload preserves fixed allocation search and manual risk overrides", () => {
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
  assert.equal(payload.dynamic_allocation_enabled, false);
  assert.equal(payload.allocation_mode, "fixed_by_risk_profile");
  assert.equal(payload.long_weight_pct, 60);
  assert.equal(payload.short_weight_pct, 40);
  assert.equal(payload.target_annualized_return_pct, 50);
  assert.equal(payload.stop_model.kind, "layer_plus_atr");
  assert.equal(payload.scoring.max_drawdown_pct, 17.5);
  assert.equal(payload.scoring.max_stop_loss_count, 9);
});

test("martingale wizard empty stop loss override falls back to risk preset defaults", () => {
  const { buildWizardPayload } = loadWizardPayloadHelpers();

  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "balanced", maxStopLossCount: "" }).scoring.max_stop_loss_count, 3);
  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "aggressive", maxStopLossCount: "" }).scoring.max_stop_loss_count, 8);
  assert.equal(buildWizardPayload({ ...baseForm, parameterPreset: "balanced", maxStopLossCount: "   " }).scoring.max_stop_loss_count, 3);
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
  assert.match(chartSource, /Long\/Short Allocation/);
  assert.match(chartSource, /long_weight_pct/);
  assert.match(chartSource, /short_weight_pct/);
  assert.match(chartSource, /btc_regime/);
  assert.match(chartSource, /symbol_regime/);
  assert.match(chartSource, /forced_exit_count/);
  assert.match(chartSource, /cost_summary/);
  assert.match(chartSource, /组合资金分配|Portfolio allocation/);
  assert.match(chartSource, /symbol_weight_pct/);
  assert.match(chartSource, /Package contributions/);
  assert.match(chartSource, /return_contribution_pct/);
  assert.match(chartSource, /drawdown_contribution_pct/);
  assert.match(resultSource, /收益回撤比|Return\/DD ratio/);
  assert.match(resultSource, /调仓次数|Rebalances/);
  assert.match(resultSource, /强平次数|Forced exits/);
  assert.match(resultSource, /交易成本|Trading cost/);
  assert.match(resultSource, /是否满足最大回撤限制|Max DD limit passed/);
  assert.match(resultSource, /是否可推荐实盘|Live recommendable/);
  assert.match(resultSource, /组合 Top10 已剔除币种|Discarded symbols from portfolio Top10/);
  assert.match(basketSource, /组合篮子|Portfolio basket/i);
  assert.match(basketSource, /权重合计|Weight total/i);
  assert.match(basketSource, /批量发布实盘组合|Batch publish live portfolio/);
});

test("backtest console normalization passes dynamic martingale metrics through", () => {
  const { normalizeCandidate } = loadBacktestConsoleHelpers();
  const normalized = normalizeCandidate({
    candidate_id: "c-1",
    status: "succeeded",
    config: {},
    summary: {
      symbol: "BTCUSDT",
      allocation_curve: [{ timestamp_ms: 1, symbol: "BTCUSDT", long_weight_pct: "60", short_weight_pct: 40 }],
      regime_timeline: [{ timestamp_ms: 1, btc_regime: "bull", symbol_regime: "range" }],
      cost_summary: { fee_quote: "1.5", slippage_quote: "bad", forced_exit_quote: 2 },
      return_drawdown_ratio: "1.8",
      rebalance_count: "4",
      forced_exit_count: "0",
      average_allocation_hold_hours: "12.5",
      live_recommended: true,
      max_drawdown_limit_passed: false,
      discarded_symbols_from_portfolio_top10: ["DOGEUSDT", 123],
    },
  }, "zh");

  assert.deepEqual(normalized.summary.allocation_curve, [{ timestamp_ms: 1, symbol: "BTCUSDT", long_weight_pct: "60", short_weight_pct: 40 }]);
  assert.deepEqual(normalized.summary.regime_timeline, [{ timestamp_ms: 1, btc_regime: "bull", symbol_regime: "range" }]);
  assert.deepEqual(normalized.summary.cost_summary, { fee_quote: 1.5, forced_exit_quote: 2 });
  assert.equal(normalized.summary.return_drawdown_ratio, 1.8);
  assert.equal(normalized.summary.rebalance_count, 4);
  assert.equal(normalized.summary.forced_exit_count, 0);
  assert.equal(normalized.summary.average_allocation_hold_hours, 12.5);
  assert.equal(normalized.summary.live_recommended, true);
  assert.equal(normalized.summary.max_drawdown_limit_passed, false);
  assert.deepEqual(normalized.summary.discarded_symbols_from_portfolio_top10, ["DOGEUSDT"]);
});

test("backtest console normalization safely falls back for empty dynamic payloads", () => {
  const { normalizeCandidate } = loadBacktestConsoleHelpers();
  const normalized = normalizeCandidate({ summary: {} }, "zh");

  assert.equal(normalized.summary.cost_summary, undefined);
  assert.equal(normalized.summary.return_drawdown_ratio, undefined);
  assert.equal(normalized.summary.profit_drawdown_ratio, undefined);
  assert.equal(normalized.summary.rebalance_count, undefined);
  assert.equal(normalized.summary.forced_exit_count, undefined);
  assert.equal(normalized.summary.average_allocation_hold_hours, undefined);
  assert.equal(normalized.summary.live_recommended, undefined);
  assert.equal(normalized.summary.can_recommend_live, undefined);
  assert.equal(normalized.summary.max_drawdown_limit_passed, undefined);
  assert.equal(normalized.summary.discarded_symbols_from_portfolio_top10.length, 0);
  assert.equal(normalized.summary.portfolio_top10_discarded_symbols.length, 0);
  assert.doesNotMatch(JSON.stringify(normalized.summary), /NaN/);
});

test("backtest chart cost helpers use top-level dynamic values and avoid NaN fallbacks", () => {
  const { normalizeCostSummary, formatCost, formatCount, formatHours } = loadBacktestChartHelpers();
  const summary = normalizeCostSummary({
    cost_summary: { fee_quote: "3.25", slippage_quote: "oops", stop_loss_quote: Number.NaN },
    rebalance_count: "6",
    forced_exit_count: "bad",
    average_allocation_hold_hours: Number.POSITIVE_INFINITY,
  });

  assert.deepEqual(summary, { fee_quote: 3.25, rebalance_count: 6 });
  assert.equal(formatCost(summary.fee_quote), "3.25 USDT");
  assert.equal(formatCost(summary.slippage_quote), "—");
  assert.equal(formatCount(summary.rebalance_count), "6");
  assert.equal(formatCount(summary.forced_exit_count), "—");
  assert.equal(formatHours(summary.average_allocation_hold_hours), "—");
  assert.doesNotMatch(`${formatCost(summary.slippage_quote)} ${formatHours(summary.average_allocation_hold_hours)}`, /NaN/);
});

test("backtest chart cost helpers safely fallback for empty cost payloads", () => {
  const { normalizeCostSummary, formatCost, formatCount, formatHours } = loadBacktestChartHelpers();

  assert.equal(normalizeCostSummary({}), undefined);
  assert.equal(normalizeCostSummary({ cost_summary: {} }), undefined);
  assert.equal(formatCost(normalizeCostSummary({})?.fee_quote), "—");
  assert.equal(formatCount(normalizeCostSummary({ cost_summary: {} })?.rebalance_count), "—");
  assert.equal(formatHours(normalizeCostSummary({})?.average_allocation_hold_hours), "—");
  assert.doesNotMatch(`${formatCost(normalizeCostSummary({})?.fee_quote)} ${formatCount(normalizeCostSummary({ cost_summary: {} })?.rebalance_count)}`, /NaN/);
});

test("frontend has proxy route for batch portfolio publish", () => {
  const route = read("apps/web/app/api/user/backtest/portfolios/publish/route.ts");
  assert.match(route, /backendPath:\s*["']\/backtest\/portfolios\/publish["']/);
  assert.match(route, /POST/);
});

test("martingale wizard exposes fixed long short defaults and manual validation", () => {
  const wizard = read("apps/web/components/backtest/backtest-wizard.tsx");
  assert.match(wizard, /conservative[\s\S]*longWeightPct:\s*80[\s\S]*shortWeightPct:\s*20/);
  assert.match(wizard, /balanced[\s\S]*longWeightPct:\s*60[\s\S]*shortWeightPct:\s*40/);
  assert.match(wizard, /aggressive[\s\S]*longWeightPct:\s*50[\s\S]*shortWeightPct:\s*50/);
  assert.match(wizard, /dynamic_allocation_enabled:\s*false/);
  assert.match(wizard, /longWeightPct/);
  assert.match(wizard, /shortWeightPct/);
  assert.match(wizard, /Long 与 Short 比例合计必须等于 100%|Long and Short weights must sum to 100%/);
});

test("martingale results explain score, annualized return, stops, and live recommendation", () => {
  const table = read("apps/web/components/backtest/backtest-result-table.tsx");
  const warning = read("apps/web/components/backtest/martingale-risk-warning.tsx");
  assert.match(table, /annualized_return_pct|annualizedReturnPct/);
  assert.match(table, /max_drawdown_pct|maxDrawdownPct/);
  assert.match(table, /stop_loss_count|stopLossCount/);
  assert.match(table, /fee_quote|feeQuote/);
  assert.match(table, /slippage_quote|slippageQuote/);
  assert.match(table, /can_recommend_live|canRecommendLive/);
  assert.match(table, /\/100/);
  assert.match(warning, /收益为负|negative return/i);
  assert.match(warning, /超过最大回撤|drawdown/i);
  assert.match(warning, /止损频率|stop/i);
  assert.match(warning, /不建议实盘|not recommend/i);
});
