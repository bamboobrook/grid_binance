import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const consoleSource = readFileSync("apps/web/components/backtest/backtest-console.tsx", "utf8");
const reviewSource = readFileSync("apps/web/components/backtest/portfolio-candidate-review.tsx", "utf8");

test("portfolio sandbox places publish basket action next to recalculate with highlighted CTA", () => {
  const recalcIndex = consoleSource.indexOf("重新计算组合表现");
  const publishIndex = consoleSource.indexOf("用作发布篮子");

  assert.ok(recalcIndex >= 0, "recalculate button should exist");
  assert.ok(publishIndex >= 0, "use as publish basket button should exist");
  assert.ok(
    Math.abs(publishIndex - recalcIndex) < 1600,
    "publish basket action should sit near the recalculate action, not in the sandbox header",
  );
  assert.match(
    consoleSource,
    /border-amber-500[^\n]+bg-amber-500\/10[^\n]+text-sm[^\n]+font-semibold/s,
    "publish basket CTA should be visually highlighted with amber border and larger text",
  );
});

test("batch publish normalizes enabled item weights before payload submission", () => {
  assert.match(reviewSource, /normalizePublishItemsWeights\(enabledItems\)/);
  assert.match(reviewSource, /items:\s*normalizedItems\.map/);
  assert.match(reviewSource, /weight_pct:\s*item\.normalizedWeightPct/);
  assert.match(reviewSource, /const normalizedTotal = normalized\.reduce/);
});
