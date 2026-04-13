import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("exchange test flow validates the current form input instead of only reading saved credentials", () => {
  const route = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/user/exchange/route.ts");
  const page = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/exchange/page.tsx");

  assert.match(route, /intent === "test"/, "exchange action route should branch explicitly for the test action");
  assert.match(route, /"\/exchange\/binance\/credentials\/test"/, "exchange test should post raw credentials to the dedicated API endpoint");
  assert.match(
    route,
    /intent === "test"[\s\S]*connection_status === "healthy"[\s\S]*"\/exchange\/binance\/credentials"/,
    "exchange test should auto-save the current credentials after a healthy validation run",
  );
  assert.match(route, /readField\(formData, "apiKey"\)/, "exchange test should read the current apiKey input");
  assert.match(route, /readField\(formData, "apiSecret"\)/, "exchange test should read the current apiSecret input");
  assert.match(route, /selectedMarkets/, "exchange route should read selected market scope from the submitted form");
  assert.doesNotMatch(route, /selected_markets:\s*\["spot", "usdm", "coinm"\]/, "exchange route should not hardcode all three markets for every credential test");

  assert.match(page, /当前测试结果|Current test result/, "exchange page should surface the last unsaved test result");
  assert.match(page, /选择测试市场|Choose market scope/, "exchange page should let the user choose which Binance markets to validate");
  assert.match(page, /name="selectedMarkets"/, "exchange page should submit the selected market scope fields");
  assert.match(page, /自动保存|auto-save/, "exchange page should explain that successful tests persist the credentials automatically");
  assert.doesNotMatch(
    page,
    /确认无误后还需要点击保存凭证|Save credentials after the checks look correct/,
    "exchange page should no longer require a second manual save after a successful test",
  );
  assert.match(page, /哪一步阻塞了合约启动|which exact exchange checks passed and which one blocks futures starts/, "exchange page should explain blocking reasons for failed checks");
});

test("exchange account read model survives partial persistence instead of looking fully unbound", () => {
  const service = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/api-server/src/services/exchange_service.rs");
  const page = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/exchange/page.tsx");

  assert.match(service, /binding_state|partial/i, "exchange service should expose a saved or partial credential state in the read model");
  assert.match(service, /find_exchange_account[\s\S]*find_exchange_credentials/, "exchange service should inspect both persisted account and credential rows when rebuilding the read model");
  assert.match(page, /binding_state|credential_state|partial/i, "exchange page should render the saved credential state even when validation metadata needs recovery");
  assert.doesNotMatch(page, /!snapshot\?\.api_key_masked[\s\S]*尚未连接/, "exchange page should not reduce the whole state machine to masked-key presence only");
});
