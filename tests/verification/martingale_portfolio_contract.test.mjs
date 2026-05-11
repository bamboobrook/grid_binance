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
