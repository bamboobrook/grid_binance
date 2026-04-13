import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("user app shell uses the route locale and only links to real product routes", () => {
  const layout = read("apps/web/app/[locale]/app/layout.tsx");
  const userShell = read("apps/web/components/shell/user-shell.tsx");
  const publicShell = read("apps/web/components/shell/public-shell.tsx");
  const sidebar = read("apps/web/components/layout/sidebar.tsx");

  assert.match(layout, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, "user app layout should receive the locale route param");
  assert.match(layout, /getUserShellSnapshot\(locale\)/, "user app layout should build shell state from the active locale");
  assert.match(layout, /<UserShell/, "user app layout should render the shared user shell");
  assert.doesNotMatch(layout, /ModernShell/, "user app layout should not rely on the experimental shell that introduced broken navigation");

  assert.match(userShell, /withLocale\(/, "user shell should localize brand and nav links");
  assert.doesNotMatch(userShell, /href="\/app\/dashboard"/, "user shell brand link should not be bare /app/dashboard");

  assert.match(publicShell, /withLocale\(/, "public shell should localize public links");
  assert.doesNotMatch(publicShell, /href="\/"/, "public shell brand link should not drop the locale prefix");

  assert.doesNotMatch(sidebar, /href=\{`\/en\$\{item\.href\}`\}/, "sidebar should not hardcode /en links");
  assert.doesNotMatch(sidebar, /\/app\/(portfolio|smart-trade|dca|settings)/, "sidebar should not link to routes that do not exist in the current app router");
});
