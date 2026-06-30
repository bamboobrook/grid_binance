import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const sourcePath = "scripts/hybrid_martingale_frontier_probe.py";

test("hybrid frontier probe preserves original C/B/A gates", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /"conservative":\s*\{"ann_min":\s*50\.0,\s*"dd_max":\s*10\.0\}/);
  assert.match(source, /"balanced":\s*\{"ann_min":\s*90\.0,\s*"dd_max":\s*20\.0\}/);
  assert.match(source, /"aggressive":\s*\{"ann_min":\s*110\.0,\s*"dd_max":\s*30\.0\}/);
  assert.match(source, /"h1_2023":\s*\(1672531200000,\s*1688169599999\)/);
  assert.match(source, /"h2_2023":\s*\(1688169600000,\s*1704067199999\)/);
  assert.match(source, /"2024":\s*\(1704067200000,\s*1735689599999\)/);
  assert.match(source, /"2025":\s*\(1735689600000,\s*1767225599999\)/);
  assert.match(source, /"2026_ytd":\s*\(1767225600000,\s*1780271999999\)/);
});

test("hybrid frontier probe cannot claim live parity in Phase 1", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /LIVE_PARITY_STATUS\s*=\s*"research_only"/);
  assert.doesNotMatch(source, /live_parity_passed/);
  assert.match(source, /Phase 1 research-only/);
});

test("hybrid frontier probe documents no-lookahead stream construction", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.match(source, /no-lookahead/);
  assert.match(source, /warmup/);
  assert.match(source, /decision timestamp uses data at or before t/);
});
