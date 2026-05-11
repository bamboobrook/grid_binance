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
  assert.match(wizardSource, /Create wizard backtest task|创建向导回测任务/);
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

  assert.match(searchSource, /groupName="symbolPool"/);
  assert.match(searchSource, /groupName="searchMode"/);
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
