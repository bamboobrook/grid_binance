import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("web app shell structure aligns with localized public, user, and admin route systems", () => {
  const requiredFiles = [
    "apps/web/app/[locale]/layout.tsx",
    "apps/web/app/[locale]/page.tsx",
    "apps/web/app/[locale]/(public)/layout.tsx",
    "apps/web/app/[locale]/(public)/login/page.tsx",
    "apps/web/app/[locale]/(public)/register/page.tsx",
    "apps/web/app/[locale]/app/layout.tsx",
    "apps/web/app/[locale]/app/strategies/new/page.tsx",
    "apps/web/app/[locale]/app/orders/page.tsx",
    "apps/web/app/[locale]/app/analytics/page.tsx",
    "apps/web/app/[locale]/app/telegram/page.tsx",
    "apps/web/app/[locale]/app/help/page.tsx",
    "apps/web/app/[locale]/app/martingale-portfolios/page.tsx",
    "apps/web/app/[locale]/app/martingale-portfolios/[id]/page.tsx",
    "apps/web/app/[locale]/admin/layout.tsx",
    "apps/web/app/[locale]/admin/memberships/page.tsx",
    "apps/web/app/[locale]/admin/deposits/page.tsx",
    "apps/web/app/[locale]/admin/strategies/page.tsx",
    "apps/web/app/[locale]/admin/sweeps/page.tsx",
    "apps/web/app/[locale]/admin/system/page.tsx",
    "apps/web/app/api/user/strategies/templates/route.ts",
    "apps/web/components/shell/public-shell.tsx",
    "apps/web/components/shell/user-shell.tsx",
    "apps/web/components/shell/admin-shell.tsx",
    "apps/web/components/shell/path-utils.ts",
    "apps/web/components/ui/status-banner.tsx",
    "apps/web/components/ui/card.tsx",
    "apps/web/components/ui/table.tsx",
    "apps/web/components/ui/form.tsx",
    "apps/web/components/ui/tabs.tsx",
    "apps/web/components/ui/chip.tsx",
    "apps/web/components/ui/dialog.tsx",
    "apps/web/lib/api/server.ts",
    "apps/web/lib/api/mock-data.ts",
    "apps/web/lib/api/help-articles.ts",
  ];

  for (const file of requiredFiles) {
    assert.ok(fs.existsSync(file), `${file} should exist`);
  }

  assert.equal(fs.existsSync("apps/web/app/page.tsx"), false, "homepage should live under the localized route tree");

  const localeLayout = read("apps/web/app/[locale]/layout.tsx");
  const publicLayout = read("apps/web/app/[locale]/(public)/layout.tsx");
  const userLayout = read("apps/web/app/[locale]/app/layout.tsx");
  const adminLayout = read("apps/web/app/[locale]/admin/layout.tsx");
  const sidebar = read("apps/web/components/layout/sidebar.tsx");
  const publicShell = read("apps/web/components/shell/public-shell.tsx");
  const userShell = read("apps/web/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/components/shell/admin-shell.tsx");
  const helpPage = read("apps/web/app/[locale]/app/help/page.tsx");
  const helpSlugPage = read("apps/web/app/[locale]/help/[slug]/page.tsx");
  const serverApi = read("apps/web/lib/api/server.ts");

  assert.match(localeLayout, /NextIntlClientProvider/);
  assert.match(localeLayout, /ThemeProvider/);
  assert.match(publicLayout, /ThemeToggle/);
  assert.match(publicLayout, /LocaleToggle/);
  assert.match(userLayout, /getUserShellSnapshot/);
  assert.match(adminLayout, /getAdminShellSnapshot/);
  assert.match(adminLayout, /getCurrentAdminProfile/);
  assert.match(sidebar, /martingale-portfolios/);
  assert.match(sidebar, /Martingale Portfolios|马丁组合/i);

  assert.match(publicShell, /usePathname/);
  assert.match(userShell, /usePathname/);
  assert.match(adminShell, /usePathname/);

  for (const [page, pattern] of [
    ["apps/web/app/[locale]/app/membership/page.tsx", /redirect\(`\/\$\{locale\}\/app\/billing`\)/],
    ["apps/web/app/[locale]/admin/billing/page.tsx", /redirect\(`\/\$\{locale\}\/admin\/deposits`\)/],
  ]) {
    assert.match(read(page), pattern, `${page} should keep the documented localized redirect`);
  }

  const notificationsPage = read("apps/web/app/[locale]/app/notifications/page.tsx");
  assert.match(notificationsPage, /AppShellSection/);
  assert.match(notificationsPage, /notifications/i);
  assert.doesNotMatch(notificationsPage, /redirect\(/, "notifications page should be a standalone inbox route");

  assert.match(helpPage, /normalizeHelpArticle/);
  assert.match(helpPage, /getHelpArticle/);
  assert.match(helpPage, /listHelpArticles/);
  assert.match(helpPage, /notFound\(/);
  assert.match(helpSlugPage, /getHelpArticle/);
  assert.match(helpSlugPage, /notFound\(/);
  assert.match(helpSlugPage, /shell--public/);
  assert.match(helpSlugPage, /\/\$\{locale\}\/app\/help\?article=\$\{article\.slug\}/);

  assert.match(serverApi, /getUserShellSnapshot/);
  assert.match(serverApi, /getAdminShellSnapshot/);
  assert.match(serverApi, /server-only/);
});

test("web shells expose persistent language and theme preferences", () => {
  const localeLayout = read("apps/web/app/[locale]/layout.tsx");
  const preferences = read("apps/web/lib/ui/preferences.ts");
  const shellPreferences = read("apps/web/components/shell/shell-preferences.tsx");
  const publicShell = read("apps/web/components/shell/public-shell.tsx");
  const userShell = read("apps/web/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/components/shell/admin-shell.tsx");

  assert.match(preferences, /UI_LANGUAGE_COOKIE/);
  assert.match(preferences, /UI_THEME_COOKIE/);
  assert.match(preferences, /buildThemeInitScript/);
  assert.match(shellPreferences, /buildPreferenceCookie/);
  assert.match(shellPreferences, /UI_LANGUAGE_COOKIE/);
  assert.match(shellPreferences, /UI_THEME_COOKIE/);
  assert.match(localeLayout, /cookies\(/, "localized root layout should read persisted UI preferences");
  assert.match(localeLayout, /resolveUiTheme|UI_THEME_COOKIE|buildThemeInitScript/, "localized root layout should hydrate theme from persisted preference state");
  assert.doesNotMatch(localeLayout, /forcedTheme="dark"/, "localized root layout should not hard-force dark theme");
  assert.match(publicShell, /ShellPreferences|Theme|主题|Language|语言/i);
  assert.match(userShell, /ShellPreferences|Theme|主题|Language|语言/i);
  assert.match(adminShell, /ShellPreferences|Theme|主题|Language|语言/i);
});

test("shared shell visual system follows a professional trading-console contract", () => {
  const globalsCss = read("apps/web/styles/globals.css");
  const publicShell = read("apps/web/components/shell/public-shell.tsx");
  const userShell = read("apps/web/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/components/shell/admin-shell.tsx");
  const section = read("apps/web/components/shell/app-shell-section.tsx");
  const card = read("apps/web/components/ui/card.tsx");
  const form = read("apps/web/components/ui/form.tsx");
  const banner = read("apps/web/components/ui/status-banner.tsx");
  const table = read("apps/web/components/ui/table.tsx");
  const chip = read("apps/web/components/ui/chip.tsx");
  const dialog = read("apps/web/components/ui/dialog.tsx");
  const tabs = read("apps/web/components/ui/tabs.tsx");

  assert.match(globalsCss, /\.shell-sidebar/);
  assert.match(globalsCss, /\.shell-topbar/);
  assert.match(globalsCss, /\.metric-strip/);
  assert.match(globalsCss, /\.market-strip/);
  assert.match(globalsCss, /\.shell-preferences/);
  assert.match(publicShell, /market-strip|shell-topbar__console|supportLinks/i);
  assert.match(userShell, /metric-strip|workspace|quickStats/i);
  assert.match(adminShell, /metric-strip|operations|quickStats/i);
  assert.match(section, /border-b border-border\/60|flex flex-col gap-4/i);
  assert.doesNotMatch(section, /text-slate-100/, "section headings should respect light and dark themes");
  assert.match(card, /ui-card__header|ui-card|CardHeader/i);
  assert.match(form, /ui-field__meta|ui-input|button/i);
  assert.match(banner, /status-banner__meta|status-banner__actions|status-banner/i);
  assert.match(table, /ui-table__scroller|ui-table|table-wrap/i);
  assert.match(chip, /UiLanguageProvider|useUiCopy|ui-chip/i);
  assert.match(dialog, /ui-dialog__header|ui-dialog__body|ui-dialog/i);
  assert.match(tabs, /ui-tab__meta|ui-tab|ui-tabs/i);
});

test("user shell and dashboard avoid fabricated placeholder identity and expose real operating stats", () => {
  const mockData = read("apps/web/lib/api/mock-data.ts");
  const serverApi = read("apps/web/lib/api/server.ts");
  const dashboardPage = read("apps/web/app/[locale]/app/dashboard/page.tsx");
  const exchangePage = read("apps/web/app/[locale]/app/exchange/page.tsx");
  const notificationsPage = read("apps/web/app/[locale]/app/notifications/page.tsx");
  const helpPage = read("apps/web/app/[locale]/app/help/page.tsx");
  const preview = read("apps/web/components/strategies/strategy-visual-preview.tsx");

  assert.doesNotMatch(mockData, /Luna Chen|13 天后续费|13 days|badge:\s*"8"|badge:\s*"3"/, "user shell mock data should not ship fabricated identity, renewal countdowns, or fake badges");
  assert.match(serverApi, /base\.identity\.name = profile\.email/, "user shell identity should be anchored to the real signed-in account");
  assert.match(dashboardPage, /Fees paid|手续费/i, "dashboard should surface fee totals");
  assert.match(dashboardPage, /Funding total|资金费/i, "dashboard should surface funding totals");
  assert.match(dashboardPage, /Membership Status|会员状态/i, "dashboard should surface membership state");
  assert.match(dashboardPage, /ErrorPaused|异常阻塞/i, "dashboard should surface error-paused visibility");
  assert.match(dashboardPage, /account_snapshots|Recent account activity|近期账户活动/i, "dashboard should show recent account activity");
  assert.match(dashboardPage, /Asset Allocation|资产分布|data-asset-allocation-chart/, "dashboard should expose a real asset allocation chart instead of a plain balance list");
  assert.match(exchangePage, /api_key_masked/, "exchange summary should branch on real credential presence");
  assert.match(exchangePage, /binding_state|credential_state|partial/i, "exchange page should keep a saved credential state even if one persisted row is temporarily missing");
  assert.doesNotMatch(exchangePage, /const supportedScopes = selectedMarkets\.map/, "exchange page should not claim supported scopes when credentials are missing");
  assert.match(notificationsPage, /created_at|时间|Timestamp/i, "notifications page should include event timestamps");
  assert.match(notificationsPage, /message|说明|Summary/i, "notifications page should include a human-readable summary or description");
  assert.match(helpPage, /humanize|sanitize|friendly/i, "help page should sanitize repository docs into user-facing language");
  assert.doesNotMatch(helpPage, /docs\/user-guide/, "help page should not expose raw repository path copy to end users");
  assert.doesNotMatch(preview, /theme: "dark"/, "TradingView preview theme should follow the current site theme");
});

test("exchange page preserves persisted credential summary even when a stale test cookie exists", () => {
  const exchangePage = read("apps/web/app/[locale]/app/exchange/page.tsx");

  assert.match(exchangePage, /const persistedSnapshot = account\?\.account \?\? null;/, "exchange page should isolate the persisted server snapshot");
  assert.match(exchangePage, /const validationSnapshot = testResult\?\.account \?\? persistedSnapshot;/, "exchange page should only use the test cookie for validation details");
  assert.match(exchangePage, /const summarySnapshot = persistedSnapshot \?\? validationSnapshot;/, "exchange page summary should prefer the saved account state over the cookie");
});

test("shared status and table copy localizes instead of leaking fixed english labels", () => {
  const banner = read("apps/web/components/ui/status-banner.tsx");
  const dialog = read("apps/web/components/ui/dialog.tsx");
  const table = read("apps/web/components/ui/table.tsx");
  const chip = read("apps/web/components/ui/chip.tsx");

  assert.match(chip, /createContext|pickText|UiLanguageProvider|useUiCopy/);
  assert.match(dialog, /pickText|useUiCopy|lang/i);
  assert.match(table, /pickText|useUiCopy|lang/i);
  assert.doesNotMatch(dialog, />\s*Warning\s*</);
  assert.doesNotMatch(table, /No records available\./);
  assert.doesNotMatch(banner, /Heads up|Warning|No records available\./, "status banner should not hardcode english helper copy");
});

test("shell and help helpers enforce route behavior", async () => {
  const { isNavHrefActive } = await import("../../apps/web/components/shell/path-utils.ts");
  const { isValidHelpArticle, normalizeHelpArticle } = await import("../../apps/web/lib/api/help-articles.ts");

  assert.equal(isNavHrefActive("/app/orders", "/app/orders"), true);
  assert.equal(isNavHrefActive("/app/strategies/grid-btc", "/app/strategies"), true);
  assert.equal(isNavHrefActive("/app/telegram", "/app/orders"), false);

  assert.equal(isValidHelpArticle("expiry-reminder"), true);
  assert.equal(isValidHelpArticle("unknown-slug"), false);
  assert.equal(normalizeHelpArticle("expiry-reminder"), "expiry-reminder");
  assert.equal(normalizeHelpArticle(["expiry-reminder", "other"]), "expiry-reminder");
  assert.equal(normalizeHelpArticle("unknown-slug"), null);
});

test("user app routes do not rely on fabricated product state for critical truth", () => {
  const dashboardPage = read("apps/web/app/[locale]/app/dashboard/page.tsx");
  const securityPage = read("apps/web/app/[locale]/app/security/page.tsx");
  const ordersPage = read("apps/web/app/[locale]/app/orders/page.tsx");
  const exchangePage = read("apps/web/app/[locale]/app/exchange/page.tsx");
  const appLayout = read("apps/web/app/[locale]/app/layout.tsx");

  assert.doesNotMatch(dashboardPage, /getUserDashboardSnapshot/);
  assert.doesNotMatch(securityPage, /getSecuritySnapshot/);
  assert.doesNotMatch(ordersPage, /getOrdersSnapshot/);
  assert.match(dashboardPage, /fetchAnalytics\(\)|fetchStrategies\(\)/);
  assert.match(securityPage, /fetchProfile\(\)/);
  assert.match(ordersPage, /fetchStrategyRuntimes|fetchAnalytics\(\)/);
  assert.match(exchangePage, /Validation details|Permissions OK|Hedge mode OK/i);
  assert.match(appLayout, /getUserExpiryNotification|DialogFrame|aria-modal="true"|show_expiry_popup/i, "user app layout should wire the expiry popup contract into the shared shell");
});

test("public landing and auth pages localize primary copy instead of single-language rendering", () => {
  const homePage = read("apps/web/app/[locale]/page.tsx");
  const loginPage = read("apps/web/app/[locale]/(public)/login/page.tsx");
  const registerPage = read("apps/web/app/[locale]/(public)/register/page.tsx");

  assert.match(homePage, /getTranslations/);
  assert.match(loginPage, /pickText|resolveUiLanguageFromRoute|cookies\(/);
  assert.match(registerPage, /pickText|resolveUiLanguageFromRoute|cookies\(/);
  assert.match(homePage, /title1|title2|subtitle/);
  assert.match(loginPage, /重置密码|Reset password/);
  assert.match(registerPage, /注册后可直接登录|Sign in right after registration/);
});

test("strategy save route does not auto-pause running strategies", () => {
  const strategyRoute = read("apps/web/app/api/user/strategies/[id]/route.ts");

  assert.match(strategyRoute, /Strategy must be paused before editing and saving changes\./);
});

test("strategy workspace exposes batch actions, templates, and multi-level payload plumbing", () => {
  const strategiesPage = read("apps/web/app/[locale]/app/strategies/page.tsx");
  const strategyInventoryTable = read("apps/web/components/strategies/strategy-inventory-table.tsx");
  const newStrategyPage = read("apps/web/app/[locale]/app/strategies/new/page.tsx");
  const detailPage = read("apps/web/app/[locale]/app/strategies/[id]/page.tsx");
  const workspaceForm = read("apps/web/components/strategies/strategy-workspace-form.tsx");
  const createRoute = read("apps/web/app/api/user/strategies/create/route.ts");
  const detailRoute = read("apps/web/app/api/user/strategies/[id]/route.ts");
  const templatesRoute = read("apps/web/app/api/user/strategies/templates/route.ts");
  const backendStrategiesRoute = read("apps/api-server/src/routes/strategies.rs");

  assert.match(strategiesPage, /\/api\/user\/strategies\/batch/);
  assert.match(strategiesPage, /stop-all/);
  assert.match(strategyInventoryTable, /批量启动|Batch Start/);
  assert.match(strategyInventoryTable, /批量暂停|Batch Pause/);
  assert.match(strategyInventoryTable, /批量删除|Batch Delete/);

  assert.match(newStrategyPage, /StrategyWorkspaceForm/);
  assert.match(newStrategyPage, /\/api\/user\/strategies\/templates/);
  assert.match(detailPage, /StrategyWorkspaceForm/);
  assert.match(detailPage, /Runtime Events|运行事件/);

  assert.match(workspaceForm, /StrategySymbolPicker/);
  assert.match(workspaceForm, /levels_json/);
  assert.match(workspaceForm, /amountMode|quoteAmount|baseQuantity|referencePrice|gridSpacingPercent|gridCount|batchTakeProfit|batchTrailing/i);
  assert.match(workspaceForm, /overallStopLoss|overallTakeProfit/);

  assert.match(createRoute, /levels_json/);
  assert.match(createRoute, /amountMode|quoteAmount|baseQuantity|referencePrice|gridSpacingPercent|gridCount|batchTakeProfit|batchTrailing/i);
  assert.match(createRoute, /JSON\.parse/);
  assert.doesNotMatch(createRoute, /levels:\s*\[\s*\{[\s\S]*\{[\s\S]*\{[\s\S]*\]/);

  assert.match(detailRoute, /levels_json/);
  assert.match(detailRoute, /overallTakeProfit|overallStopLoss/);
  assert.match(templatesRoute, /\/strategies\/templates/);
  assert.match(templatesRoute, /\/apply/);
  assert.match(backendStrategiesRoute, /\/strategies\/templates/);
  assert.match(backendStrategiesRoute, /\/strategies\/templates\/\{template_id\}\/apply/);
});

test("billing page surfaces address assignment, queue, and lock timing semantics", () => {
  const billingPage = read("apps/web/app/[locale]/app/billing/page.tsx");
  const billingRoute = read("apps/web/app/api/user/billing/route.ts");

  assert.match(billingPage, /Assigned address|分配地址|Address lock expires|锁定到期|Queue position|排队序号/i);
  assert.match(billingPage, /queue_position|address|expires_at/i);
  assert.match(billingRoute, /expires_at|queue_position/i);
  assert.match(billingRoute, /address/i);
});

test("telegram page distinguishes delivered, failed, web-only, and not-bound outcomes", () => {
  const telegramPage = read("apps/web/app/[locale]/app/telegram/page.tsx");

  assert.match(telegramPage, /Delivered|已送达/);
  assert.match(telegramPage, /Failed|失败/);
  assert.match(telegramPage, /Web only|仅站内/i);
  assert.match(telegramPage, /Not bound|未绑定/i);
});

test("user workflow guides explain batch editing, restart rules, and billing remediation", () => {
  const createGuide = read("docs/user-guide/create-grid-strategy.md");
  const manageGuide = read("docs/user-guide/manage-strategy.md");
  const billingGuide = read("docs/user-guide/membership-and-payment.md");

  assert.match(createGuide, /Amount mode|Quote amount|Base asset quantity/i);
  assert.match(createGuide, /Batch spacing|Batch take profit|逐格自定义/i);
  assert.match(manageGuide, /Start selected|Pause selected|Delete selected|Stop all/i);
  assert.match(manageGuide, /Pause, save, re-run pre-flight, then restart/i);
  assert.match(billingGuide, /Assigned address|Queue position|address lock/i);
  assert.match(billingGuide, /manual review|grace period|exact amount/i);
});
