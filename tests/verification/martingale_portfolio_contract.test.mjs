import test from "node:test";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { readFileSync } from "node:fs";

const proxyHelperUrl = pathToFileURL(
  path.resolve("apps/web/app/api/user/backtest/proxy.ts"),
).href;

const requestClientUrl = pathToFileURL(
  path.resolve("apps/web/components/backtest/request-client.ts"),
).href;

const liveControlsUtilsUrl = pathToFileURL(
  path.resolve("apps/web/components/backtest/live-portfolio-controls-utils.ts"),
).href;

test("martingale portfolio proxy routes preserve method, body, query, cookie, and status", async () => {
  const { proxyBacktestRequest } = await import(proxyHelperUrl);

  let fetchCall = null;
  const originalFetch = global.fetch;
  global.fetch = async (url, init) => {
    fetchCall = { url, init };
    return new Response(JSON.stringify({ ok: true, route: "martingale", echoed: init.body }), {
      status: 202,
      headers: { "content-type": "application/json; charset=utf-8" },
    });
  };

  try {
    const request = new Request("http://localhost/api/user/martingale-portfolios/mp_1/pause?reason=risk", {
      method: "POST",
      headers: {
        cookie: "session_token=session-789; ui_lang=zh",
        "content-type": "application/json",
      },
      body: JSON.stringify({ pause_new_entries: true }),
    });

    const response = await proxyBacktestRequest(request, {
      backendPath: "/martingale-portfolios/mp_1/pause",
    });
    const payload = await response.json();

    assert.equal(response.status, 202);
    assert.deepEqual(payload, {
      ok: true,
      route: "martingale",
      echoed: JSON.stringify({ pause_new_entries: true }),
    });
    assert.equal(fetchCall.url, "http://127.0.0.1:8080/martingale-portfolios/mp_1/pause?reason=risk");
    assert.equal(fetchCall.init.method, "POST");
    assert.equal(fetchCall.init.body, JSON.stringify({ pause_new_entries: true }));
    assert.equal(fetchCall.init.headers.get("authorization"), "Bearer session-789");
    assert.equal(fetchCall.init.headers.get("cookie"), "session_token=session-789; ui_lang=zh");
  } finally {
    global.fetch = originalFetch;
  }
});

test("martingale portfolio proxy preserves non-json upstream errors", async () => {
  const { proxyBacktestRequest } = await import(proxyHelperUrl);

  const originalFetch = global.fetch;
  global.fetch = async () => new Response("portfolio runtime unavailable", {
    status: 503,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });

  try {
    const request = new Request("http://localhost/api/user/martingale-portfolios/mp_9/stop", {
      method: "POST",
      headers: {
        authorization: "Bearer direct-token",
      },
    });

    const response = await proxyBacktestRequest(request, {
      backendPath: "/martingale-portfolios/mp_9/stop",
    });

    assert.equal(response.status, 503);
    assert.equal(await response.text(), "portfolio runtime unavailable");
    assert.match(response.headers.get("content-type") ?? "", /text\/plain/i);
  } finally {
    global.fetch = originalFetch;
  }
});

test("requestBacktestApi converts network failures into structured errors", async () => {
  const { requestBacktestApi } = await import(requestClientUrl);

  const originalFetch = global.fetch;
  global.fetch = async () => {
    throw new Error("socket hang up");
  };

  try {
    const result = await requestBacktestApi("/api/user/martingale-portfolios");
    assert.equal(result.ok, false);
    assert.equal(result.status, 0);
    assert.equal(result.text, "");
    assert.equal(result.data, null);
    assert.match(result.message, /socket hang up/i);
  } finally {
    global.fetch = originalFetch;
  }
});

test("martingale portfolio controls source guarantees loading cleanup, pending cleanup, and honest status copy", () => {
  const source = readFileSync("apps/web/components/backtest/live-portfolio-controls.tsx", "utf8");
  const sidebar = readFileSync("apps/web/components/layout/sidebar.tsx", "utf8");

  assert.match(source, /finally\s*\{[\s\S]*setLoading\(false\)/, "load flows should clear loading in finally");
  assert.match(source, /finally\s*\{[\s\S]*setPending\(\"\"\)/, "action flows should clear pending in finally");
  assert.match(source, /继承组合状态|Inherited portfolio status/i, "strategy state should disclose inherited fallback status");
  assert.match(source, /本地临时状态|Local temporary status/i, "strategy action result should be marked as temporary local state");
  assert.match(source, /等待后端同步|awaiting backend sync/i, "UI should disclose backend sync lag");
  assert.match(source, /aria-live="polite"|role="status"/, "status messages should be announced accessibly");
  assert.match(
    source,
    /\{entity\.kind === "portfolio" && entity\.status === "running" \?/,
    "pause-new-entries control should only be offered while the portfolio is running",
  );
  assert.match(sidebar, /aria-current=\{active \? "page" : undefined\}/, "active sidebar link should expose aria-current");
});

test("martingale batch portfolio publish API contract is wired end to end", async () => {
  const { canDirectPublish, canSaveDraft } = await import(liveControlsUtilsUrl);
  const routesSource = readFileSync("apps/api-server/src/routes/backtest.rs", "utf8");
  const publishServiceSource = readFileSync("apps/api-server/src/services/martingale_publish_service.rs", "utf8");
  const liveControlsSource = readFileSync("apps/web/components/backtest/live-portfolio-controls.tsx", "utf8");
  const liveControlsUtilsSource = readFileSync("apps/web/components/backtest/live-portfolio-controls-utils.ts", "utf8");
  const candidateReviewSource = readFileSync("apps/web/components/backtest/portfolio-candidate-review.tsx", "utf8");

  assert.match(routesSource, /\/backtest\/portfolios\/publish/);
  assert.match(publishServiceSource, /struct\s+PublishPortfolioRequest[\s\S]*total_weight_pct[\s\S]*items/);
  assert.match(publishServiceSource, /dynamic_allocation_rules/);
  assert.match(publishServiceSource, /live_readiness_blockers/);
  assert.match(publishServiceSource, /live_ready/);
  assert.match(publishServiceSource, /requires_dynamic_allocation_rules\(request,\s*candidates_by_id\)/);
  assert.match(publishServiceSource, /dynamic_allocation_enabled/);
  assert.match(publishServiceSource, /direction_mode/);
  assert.match(publishServiceSource, /validate_live_ready_for_start\(&portfolio\)\?/);
  assert.match(publishServiceSource, /struct\s+PublishPortfolioItemRequest[\s\S]*candidate_id[\s\S]*weight_pct[\s\S]*leverage/);
  assert.match(publishServiceSource, /struct\s+PublishPortfolioResponse[\s\S]*instances:\s*Vec<PublishedStrategyInstance>[\s\S]*items:\s*Vec<PublishedStrategyInstance>/);
  assert.match(publishServiceSource, /strategy_instance_id/);
  assert.match(candidateReviewSource, /function\s+readDynamicAllocationRules/);
  assert.match(candidateReviewSource, /const\s+dynamicAllocationRules\s*=\s*readDynamicAllocationRules\(enabledItems\)/);
  assert.match(
    candidateReviewSource,
    /const\s+payload\s*=\s*\{[\s\S]*total_weight_pct:\s*100,[\s\S]*dynamic_allocation_rules:\s*dynamicAllocationRules[\s\S]*items:/,
    "portfolio-candidate-review must place dynamic_allocation_rules at the top level of the real publish payload",
  );
  assert.match(liveControlsSource, /dynamic_allocation_rules/);
  assert.match(liveControlsSource, /liveReadinessBlockers/);
  assert.match(liveControlsSource, /实盘就绪阻断项|Live readiness blockers/);
  assert.match(liveControlsSource, /直接发布实盘|Direct live publish/);
  assert.match(liveControlsSource, /保存为待启用组合|Save as pending portfolio/);
  assert.match(liveControlsSource, /canDirectPublish/);
  assert.match(liveControlsUtilsSource, /export\s+function\s+canDirectPublish/);
  assert.match(liveControlsUtilsSource, /export\s+function\s+canSaveDraft/);
  assert.equal(canDirectPublish("", []), true);
  assert.equal(canDirectPublish("", ["dynamic allocation rules are required"]), false);
  assert.equal(canSaveDraft(""), true);
  assert.equal(canSaveDraft("start"), false);
  assert.match(liveControlsSource, /disabled=\{directLiveDisabled\}/);
  assert.match(liveControlsSource, /保存为待启用组合[\s\S]*disabled=\{!canSaveDraft\(pending\)\}/);
  assert.match(liveControlsSource, /策略实例|Strategy instance/);
  assert.match(liveControlsSource, /来源候选|Source candidate/);
});

test("martingale publish service blocks non-recommended backtest candidates", () => {
  const service = readFileSync("apps/api-server/src/services/martingale_publish_service.rs", "utf8");
  assert.match(service, /can_recommend_live/);
  assert.match(service, /max_drawdown_limit_passed/);
  assert.match(service, /not recommended|不建议|cannot publish|risk/i);
});
