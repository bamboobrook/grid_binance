import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

test("backtest console exposes dedicated martingale workflow surface", () => {
  const page = readFileSync("apps/web/app/[locale]/app/backtest/page.tsx", "utf8");
  assert.match(page, /BacktestConsole/);
  assert.match(page, /locale=\{locale\}/);
  assert.ok(existsSync("apps/web/components/backtest/backtest-console.tsx"));

  const consoleSource = readFileSync("apps/web/components/backtest/backtest-console.tsx", "utf8");
  for (const text of ["随机搜索", "智能搜索", "Hedge Mode", "逐仓", "全仓", "Portfolio", "生存优先"]) {
    assert.match(consoleSource, new RegExp(text));
  }
  assert.match(consoleSource, /role="tablist"/);
  assert.match(consoleSource, /role="tab"/);
  assert.match(consoleSource, /aria-selected=/);
  assert.match(consoleSource, /aria-controls=/);
  assert.match(consoleSource, /role="tabpanel"/);
  assert.match(consoleSource, /aria-labelledby=/);
});

test("backtest console interactions stay in-page and use client fetch", () => {
  const professionalSource = readFileSync(
    "apps/web/components/backtest/backtest-professional-panel.tsx",
    "utf8",
  );
  const taskListSource = readFileSync(
    "apps/web/components/backtest/backtest-task-list.tsx",
    "utf8",
  );
  const reviewSource = readFileSync(
    "apps/web/components/backtest/portfolio-candidate-review.tsx",
    "utf8",
  );
  const searchSource = readFileSync(
    "apps/web/components/backtest/search-config-editor.tsx",
    "utf8",
  );
  const requestClientSource = readFileSync(
    "apps/web/components/backtest/request-client.ts",
    "utf8",
  );
  const wizardSource = readFileSync(
    "apps/web/components/backtest/backtest-wizard.tsx",
    "utf8",
  );

  assert.match(professionalSource, /^"use client";/m);
  assert.match(taskListSource, /^"use client";/m);
  assert.match(reviewSource, /^"use client";/m);

  assert.match(professionalSource, /requestBacktestApi\("\/api\/user\/backtest\/tasks"/);
  assert.match(wizardSource, /requestBacktestApi\("\/api\/user\/backtest\/tasks"/);
  assert.match(wizardSource, /Start backtest|启动回测/);
  assert.match(taskListSource, /requestBacktestApi\(`\/api\/user\/backtest\/tasks\/\$\{id\}\/\$\{action\}`/);
  assert.match(reviewSource, /requestBacktestApi\(`\/api\/user\/backtest\/candidates\/\$\{candidate\.id\}\/publish-intent`/);
  assert.match(reviewSource, /portfolio_id/);
  assert.match(reviewSource, /`\/\$\{locale\}\/app\/martingale-portfolios`/);
  assert.match(requestClientSource, /fetch\(input, init\)/);

  const consoleSource = readFileSync(
    "apps/web/components/backtest/backtest-console.tsx",
    "utf8",
  );
  assert.match(consoleSource, /requestBacktestApi\("\/api\/user\/backtest\/tasks"/);
  assert.match(consoleSource, /requestBacktestApi\(`\/api\/user\/backtest\/tasks\/\$\{taskId\}\/candidates`/);
  assert.doesNotMatch(consoleSource, /SAMPLE_TASKS|SAMPLE_CANDIDATES|cand-btc-a/);

  assert.match(professionalSource, /preventDefault\(\)/);
  assert.doesNotMatch(professionalSource, /action="\/api\/user\/backtest\/tasks"/);
  assert.doesNotMatch(reviewSource, /method="post"/i);

  assert.match(searchSource, /name="symbolPoolMode"/);
  assert.match(searchSource, /name="searchMode"/);
});


test("backtest and martingale portfolio pages are reachable from user navigation", () => {
  const mockData = readFileSync("apps/web/lib/api/mock-data.ts", "utf8");
  const userShell = readFileSync("apps/web/components/shell/user-shell.tsx", "utf8");
  const sidebar = readFileSync("apps/web/components/layout/sidebar.tsx", "utf8");
  const mobileBottomNav = readFileSync("apps/web/components/layout/mobile-bottom-nav.tsx", "utf8");

  assert.match(mockData, /href: "\/app\/backtest"/);
  assert.match(mockData, /href: "\/app\/martingale-portfolios"/);
  assert.match(mockData, /回测|Backtest/);
  assert.match(userShell, /href\.includes\("backtest"\)/);
  assert.match(userShell, /href\.includes\("martingale-portfolios"\)/);
  assert.match(sidebar, /href: '\/app\/backtest'/);
  assert.match(sidebar, /href: '\/app\/martingale-portfolios'/);
  assert.match(mobileBottomNav, /href: "\/app\/backtest"/);
});


test("user shell shows backtest labels without requiring sidebar hover", () => {
  const userShell = readFileSync("apps/web/components/shell/user-shell.tsx", "utf8");
  const mockData = readFileSync("apps/web/lib/api/mock-data.ts", "utf8");

  assert.match(userShell, /useState\(true\)/, "user sidebar should default expanded so labels are visible");
  assert.ok(
    mockData.indexOf('href: "/app/backtest"') < mockData.indexOf('href: "/app/orders"'),
    "backtest should be in the core trading nav before orders/analytics",
  );
  assert.ok(
    mockData.indexOf('href: "/app/martingale-portfolios"') < mockData.indexOf('href: "/app/orders"'),
    "martingale portfolios should be in the core trading nav before orders/analytics",
  );
});

test("backtest wizard is a real editable launcher, not a static template", () => {
  const wizardSource = readFileSync(
    "apps/web/components/backtest/backtest-wizard.tsx",
    "utf8",
  );
  const martingaleSource = readFileSync(
    "apps/web/components/backtest/martingale-parameter-editor.tsx",
    "utf8",
  );
  const searchSource = readFileSync(
    "apps/web/components/backtest/search-config-editor.tsx",
    "utf8",
  );
  const timeSource = readFileSync(
    "apps/web/components/backtest/time-split-editor.tsx",
    "utf8",
  );
  const riskSource = readFileSync(
    "apps/web/components/backtest/risk-rule-editor.tsx",
    "utf8",
  );

  assert.doesNotMatch(wizardSource, /const WIZARD_PAYLOAD/);
  assert.match(wizardSource, /buildWizardPayload\(/);
  assert.match(wizardSource, /resolveAutoTimeSplit\(/);
  assert.match(wizardSource, /presetSearchSpaces/);
  assert.match(wizardSource, /parameterPreset/);
  assert.match(wizardSource, /timeMode: "auto_recent"/);
  assert.match(wizardSource, /per_symbol_top_n: 5/);
  assert.match(wizardSource, /risk_profile: form\.parameterPreset/);
  assert.match(wizardSource, /lastDayOfPreviousMonth/);
  assert.match(wizardSource, /trainStart: "2023-01-01"/);
  assert.match(wizardSource, /portfolio_basket/);
  assert.match(wizardSource, /spacing_bps/);
  assert.match(wizardSource, /first_order_quote/);
  assert.match(wizardSource, /order_multiplier/);
  assert.match(wizardSource, /parseSymbolList\(/);
  assert.match(wizardSource, /MAX_SYMBOLS = 20/);
  assert.match(wizardSource, /白名单最多支持 20 个币种|up to 20 symbols/);
  assert.match(wizardSource, /JSON\.stringify\(buildWizardPayload\(form, indicators, scoringWeights\)\)/);
  assert.match(wizardSource, /indicatorConfigsForPayload/);
  assert.match(wizardSource, /entryTriggersForPayload/);
  assert.match(wizardSource, /indicator_expression/);
  assert.match(wizardSource, /return \["immediate"\]/);
  assert.match(wizardSource, /weights: scoringWeights/);
  assert.match(wizardSource, /启动回测|Start backtest/);

  assert.match(searchSource, /textarea[\s\S]*name="whitelist"/);
  assert.match(searchSource, /textarea[\s\S]*name="blacklist"/);
  assert.match(searchSource, /value=\{form\.whitelist\}/);
  assert.match(searchSource, /value=\{form\.blacklist\}/);
  assert.match(searchSource, /onChange=\{onChange\}/);
  assert.match(searchSource, /最多 20 个币种|up to 20 symbols/);

  assert.match(martingaleSource, /SelectField[\s\S]*name="market"/);
  assert.match(martingaleSource, /SelectField[\s\S]*name="directionMode"/);
  assert.match(martingaleSource, /SelectField[\s\S]*name="marginMode"/);
  assert.match(martingaleSource, /name="initialOrderUsdt"/);
  assert.match(martingaleSource, /name="spacingPct"/);
  assert.match(martingaleSource, /name="orderMultiplier"/);
  assert.match(martingaleSource, /name="maxLegs"/);
  assert.match(martingaleSource, /name="takeProfitPct"/);
  assert.match(martingaleSource, /name="parameterPreset"/);
  assert.match(martingaleSource, /保守|Conservative/);
  assert.match(martingaleSource, /均衡|Balanced/);
  assert.match(martingaleSource, /激进|Aggressive/);

  assert.match(timeSource, /type="date"/);
  assert.match(timeSource, /name="timeMode"/);
  assert.match(timeSource, /自动最近区间|Automatic recent/);
  assert.match(timeSource, /name="trainStart"/);
  assert.match(timeSource, /name="testEnd"/);

  assert.match(riskSource, /name="maxDrawdownPct"/);
  assert.match(riskSource, /name="maxStopLossCount"/);
  assert.match(riskSource, /name="portfolioStopLossPct"/);
  assert.match(riskSource, /name="perStrategyStopLossPct"/);

  const indicatorSource = readFileSync(
    "apps/web/components/backtest/indicator-rule-editor.tsx",
    "utf8",
  );
  const scoringSource = readFileSync(
    "apps/web/components/backtest/scoring-weight-editor.tsx",
    "utf8",
  );
  assert.match(indicatorSource, /kind: "atr"/);
  assert.match(indicatorSource, /kind: "rsi"/);
  assert.match(indicatorSource, /onChange\(payload\)/);
  assert.match(scoringSource, /weight_stop_frequency/);
  assert.match(scoringSource, /weight_capital_utilization/);
  assert.match(scoringSource, /weight_trade_stability/);
  assert.doesNotMatch(scoringSource, /weight_survival/);

  const parameterSource = readFileSync(
    "apps/web/components/backtest/martingale-parameter-editor.tsx",
    "utf8",
  );
  assert.match(parameterSource, /移动止盈回撤|Moving take-profit retracement/);
  assert.match(parameterSource, /不是止损|not a stop loss/i);

  const reviewSource = readFileSync(
    "apps/web/components/backtest/portfolio-candidate-review.tsx",
    "utf8",
  );
  assert.match(reviewSource, /组合篮子|Portfolio basket/);
  assert.match(reviewSource, /权重合计|Weight total/);
  assert.match(reviewSource, /recommended_weight_pct/);
  assert.match(reviewSource, /recommended_leverage/);
});
