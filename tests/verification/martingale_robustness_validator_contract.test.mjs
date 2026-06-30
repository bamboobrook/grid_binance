import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

test("martingale robustness full-period gate honors explicit budget caps", () => {
  const source = readFileSync("scripts/validate_martingale_portfolio_robustness.py", "utf8");
  const evaluateGate = source.match(/def evaluate_gate\(metrics: dict, ann_min: float, dd_max: float\) -> bool:[\s\S]*?\n\n\ndef /);

  assert.ok(evaluateGate, "evaluate_gate helper should exist");
  assert.match(evaluateGate[0], /budget = metrics\.get\("budget"\)/);
  assert.match(evaluateGate[0], /within_budget = True if budget is None else \(metrics\["max_capital_used"\] or 0\) <= budget/);
  assert.match(evaluateGate[0], /and within_budget/);
  assert.doesNotMatch(evaluateGate[0], /max_capital_used"\] or 0\) <= 0/);
});
