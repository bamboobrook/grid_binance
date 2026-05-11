import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

test("backtest console exposes dedicated martingale workflow surface", () => {
  const page = readFileSync("apps/web/app/[locale]/app/backtest/page.tsx", "utf8");
  assert.match(page, /BacktestConsole/);
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

  assert.match(professionalSource, /^"use client";/m);
  assert.match(taskListSource, /^"use client";/m);
  assert.match(reviewSource, /^"use client";/m);

  assert.match(professionalSource, /requestBacktestApi\("\/api\/user\/backtest\/tasks"/);
  assert.match(taskListSource, /requestBacktestApi\(`\/api\/user\/backtest\/tasks\/\$\{id\}\/\$\{action\}`/);
  assert.match(reviewSource, /requestBacktestApi\(`\/api\/user\/backtest\/candidates\/\$\{candidate\.id\}\/publish-intent`/);
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
