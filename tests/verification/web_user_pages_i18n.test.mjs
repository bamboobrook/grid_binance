import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

const routeLocalizedPages = [
  "apps/web/app/[locale]/app/analytics/page.tsx",
  "apps/web/app/[locale]/app/help/page.tsx",
  "apps/web/app/[locale]/app/orders/page.tsx",
  "apps/web/app/[locale]/app/security/page.tsx",
  "apps/web/app/[locale]/app/telegram/page.tsx",
];

test("user pages resolve ui language from the locale route instead of cookie-only rendering", () => {
  for (const path of routeLocalizedPages) {
    const source = read(path);
    assert.match(source, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, `${path} should read locale from route params`);
    assert.match(source, /resolveUiLanguageFromRoute\(/, `${path} should prefer the route locale`);
  }
});

test("billing and exchange pages keep bilingual copy instead of English-only operator text", () => {
  const billingPage = read("apps/web/app/[locale]/app/billing/page.tsx");
  const exchangePage = read("apps/web/app/[locale]/app/exchange/page.tsx");
  const strategyDetailPage = read("apps/web/app/[locale]/app/strategies/[id]/page.tsx");

  for (const [path, source] of [["billing", billingPage], ["exchange", exchangePage]]) {
    assert.match(source, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, `${path} page should receive locale params`);
    assert.match(source, /pickText\(/, `${path} page should keep paired zh\/en copy`);
  }

  assert.match(billingPage, /创建支付订单|Create payment order/, "billing page should keep paired payment-order copy");
  assert.match(exchangePage, /绑定币安账户|Bind Binance account/, "exchange page should keep paired exchange credential copy");
  assert.doesNotMatch(exchangePage, /detail: "未绑定 \/ Not connected yet"/, "exchange page should not hardcode mixed-language slash strings inside one state label");
  assert.doesNotMatch(strategyDetailPage, /return `\$\{zh\} \/ \\\$\{en\}`|const bi =/, "strategy detail page should not hard-splice zh/en copy into one label");
});
