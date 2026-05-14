import test from "node:test";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { readFileSync } from "node:fs";

const helperUrl = pathToFileURL(
  path.resolve("apps/web/app/api/user/backtest/proxy.ts"),
).href;

test("backtest proxy helper forwards query, body, cookie, and status", async () => {
  const { proxyBacktestRequest } = await import(helperUrl);

  let fetchCall = null;
  const originalFetch = global.fetch;
  global.fetch = async (url, init) => {
    fetchCall = { url, init };
    return new Response(JSON.stringify({ ok: true, echoed: init.body }), {
      status: 201,
      headers: { "content-type": "application/json; charset=utf-8" },
    });
  };

  try {
    const request = new Request("http://localhost/api/user/backtest/tasks?limit=5&cursor=abc", {
      method: "POST",
      headers: {
        authorization: "Bearer upstream-token",
        cookie: "session_token=session-123; ui_lang=zh",
        "content-type": "application/json",
      },
      body: JSON.stringify({ hello: "world" }),
    });

    const response = await proxyBacktestRequest(request, {
      backendPath: "/backtest/tasks",
    });
    const payload = await response.json();

    assert.equal(response.status, 201);
    assert.deepEqual(payload, { ok: true, echoed: JSON.stringify({ hello: "world" }) });
    assert.equal(fetchCall.url, "http://127.0.0.1:8080/backtest/tasks?limit=5&cursor=abc");
    assert.equal(fetchCall.init.method, "POST");
    assert.equal(fetchCall.init.body, JSON.stringify({ hello: "world" }));
    assert.equal(fetchCall.init.headers.get("authorization"), "Bearer upstream-token");
    assert.equal(fetchCall.init.headers.get("cookie"), "session_token=session-123; ui_lang=zh");
    assert.equal(fetchCall.init.headers.get("content-type"), "application/json");
  } finally {
    global.fetch = originalFetch;
  }
});

test("backtest proxy helper derives authorization from session_token and preserves non-json errors", async () => {
  const { proxyBacktestRequest } = await import(helperUrl);

  let fetchCall = null;
  const originalFetch = global.fetch;
  global.fetch = async (url, init) => {
    fetchCall = { url, init };
    return new Response("upstream exploded", {
      status: 502,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  };

  try {
    const request = new Request("http://localhost/api/user/backtest/tasks/task-1/pause?reason=review", {
      method: "POST",
      headers: {
        cookie: "theme=dark; session_token=derived-456",
      },
    });

    const response = await proxyBacktestRequest(request, {
      backendPath: "/backtest/tasks/task-1/pause",
    });
    const text = await response.text();

    assert.equal(response.status, 502);
    assert.equal(text, "upstream exploded");
    assert.equal(fetchCall.url, "http://127.0.0.1:8080/backtest/tasks/task-1/pause?reason=review");
    assert.equal(fetchCall.init.headers.get("authorization"), "Bearer derived-456");
    assert.equal(fetchCall.init.headers.get("cookie"), "theme=dark; session_token=derived-456");
    assert.match(response.headers.get("content-type") ?? "", /text\/plain/i);
  } finally {
    global.fetch = originalFetch;
  }
});

test("backtest task routes use the shared proxy helper", () => {
  const taskRoute = readFileSync("apps/web/app/api/user/backtest/tasks/route.ts", "utf8");
  const detailRoute = readFileSync("apps/web/app/api/user/backtest/tasks/[id]/route.ts", "utf8");
  const pauseRoute = readFileSync("apps/web/app/api/user/backtest/tasks/[id]/pause/route.ts", "utf8");
  const archiveRoute = readFileSync("apps/web/app/api/user/backtest/tasks/[id]/archive/route.ts", "utf8");
  const publishIntentRoute = readFileSync(
    "apps/web/app/api/user/backtest/candidates/[id]/publish-intent/route.ts",
    "utf8",
  );

  assert.match(detailRoute, /export async function DELETE/);
  assert.match(detailRoute, /backendPath: `\/backtest\/tasks\/\$\{id\}`/);
  assert.match(archiveRoute, /export async function POST/);
  assert.match(archiveRoute, /backendPath: `\/backtest\/tasks\/\$\{id\}\/archive`/);

  for (const source of [taskRoute, detailRoute, pauseRoute, archiveRoute, publishIntentRoute]) {
    assert.match(source, /proxyBacktestRequest/);
    assert.doesNotMatch(source, /buildJsonResponse|readSessionToken|authApiBaseUrl/);
  }
});
