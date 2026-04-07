import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function escapePattern(input) {
  return input.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function loadTsExports(path, exportNames) {
  const source = read(path)
    .replace(/\sas const/g, "")
    .replace(/\sas \(typeof VALID_HELP_ARTICLES\)\[number\]/g, "")
    .replace(/\?:/g, ":")
    .replace(/: string \| string\[\]/g, "")
    .replace(/: string \| null/g, "")
    .replace(/: string/g, "")
    .replace(/: boolean/g, "")
    .replace(/export const /g, "const ")
    .replace(/export function /g, "function ");

  return new Function(`${source}\nreturn { ${exportNames.join(", ")} };`)();
}

test("web app shell structure aligns with shared public, user, and admin route systems", () => {
  const requiredFiles = [
    "apps/web/src/app/(public)/page.tsx",
    "apps/web/src/app/(public)/layout.tsx",
    "apps/web/src/app/app/layout.tsx",
    "apps/web/src/app/admin/layout.tsx",
    "apps/web/src/app/api/user/strategies/templates/route.ts",
    "apps/web/src/app/app/strategies/new/page.tsx",
    "apps/web/src/app/app/orders/page.tsx",
    "apps/web/src/app/app/analytics/page.tsx",
    "apps/web/src/app/app/telegram/page.tsx",
    "apps/web/src/app/app/help/page.tsx",
    "apps/web/src/app/admin/memberships/page.tsx",
    "apps/web/src/app/admin/deposits/page.tsx",
    "apps/web/src/app/admin/strategies/page.tsx",
    "apps/web/src/app/admin/sweeps/page.tsx",
    "apps/web/src/app/admin/system/page.tsx",
    "apps/web/src/components/shell/public-shell.tsx",
    "apps/web/src/components/shell/user-shell.tsx",
    "apps/web/src/components/shell/admin-shell.tsx",
    "apps/web/src/components/shell/path-utils.ts",
    "apps/web/src/components/ui/status-banner.tsx",
    "apps/web/src/components/ui/card.tsx",
    "apps/web/src/components/ui/table.tsx",
    "apps/web/src/components/ui/form.tsx",
    "apps/web/src/components/ui/tabs.tsx",
    "apps/web/src/components/ui/chip.tsx",
    "apps/web/src/components/ui/dialog.tsx",
    "apps/web/src/lib/api/server.ts",
    "apps/web/src/lib/api/mock-data.ts",
    "apps/web/src/lib/api/help-articles.ts",
  ];

  for (const file of requiredFiles) {
    assert.ok(fs.existsSync(file), `${file} should exist`);
  }

  assert.equal(fs.existsSync("apps/web/src/app/page.tsx"), false, "homepage should be owned by /(public)/page.tsx, not app/page.tsx");

  const homePage = read("apps/web/src/app/(public)/page.tsx");
  const publicLayout = read("apps/web/src/app/(public)/layout.tsx");
  const userLayout = read("apps/web/src/app/app/layout.tsx");
  const adminLayout = read("apps/web/src/app/admin/layout.tsx");
  const publicShell = read("apps/web/src/components/shell/public-shell.tsx");
  const userShell = read("apps/web/src/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/src/components/shell/admin-shell.tsx");
  const mockData = read("apps/web/src/lib/api/mock-data.ts");
  const helpPage = read("apps/web/src/app/app/help/page.tsx");
  const helpSlugPage = read("apps/web/src/app/help/[slug]/page.tsx");
  const analyticsPage = read("apps/web/src/app/app/analytics/page.tsx");

  assert.doesNotMatch(homePage, /PublicShell/, "homepage content should be wrapped by the public layout instead of re-rendering PublicShell");
  assert.doesNotMatch(homePage, /<main[\s>]/, "homepage should rely on shared public shell layout");
  assert.match(publicLayout, /PublicShell/);
  assert.doesNotMatch(userLayout, /\/app\/dashboard/, "user layout must not hardcode dashboard as active state");
  assert.doesNotMatch(adminLayout, /\/admin\/dashboard/, "admin layout must not hardcode dashboard as active state");

  assert.match(publicShell, /usePathname/);
  assert.match(userShell, /usePathname/);
  assert.match(adminShell, /usePathname/);

  const legacyRedirects = [
    ["apps/web/src/app/app/membership/page.tsx", /redirect\("\/app\/billing"\)/],
    ["apps/web/src/app/app/notifications/page.tsx", /redirect\("\/app\/telegram"\)/],
    ["apps/web/src/app/admin/billing/page.tsx", /redirect\("\/admin\/deposits"\)/],
  ];

  assert.doesNotMatch(analyticsPage, /redirect\("\/app\/orders"\)/, "analytics page should be a first-class route now");

  for (const [page, pattern] of legacyRedirects) {
    const source = read(page);
    assert.match(source, pattern, `${page} should redirect to the documented route`);
    assert.doesNotMatch(source, /<main[\s>]/, `${page} should not render route-local markup`);
  }

  const routePages = [
    "apps/web/src/app/(public)/login/page.tsx",
    "apps/web/src/app/(public)/register/page.tsx",
    "apps/web/src/app/(public)/page.tsx",
    "apps/web/src/app/app/dashboard/page.tsx",
    "apps/web/src/app/app/exchange/page.tsx",
    "apps/web/src/app/app/strategies/page.tsx",
    "apps/web/src/app/app/strategies/new/page.tsx",
    "apps/web/src/app/app/strategies/[id]/page.tsx",
    "apps/web/src/app/app/orders/page.tsx",
    "apps/web/src/app/app/analytics/page.tsx",
    "apps/web/src/app/app/billing/page.tsx",
    "apps/web/src/app/app/telegram/page.tsx",
    "apps/web/src/app/app/security/page.tsx",
    "apps/web/src/app/app/help/page.tsx",
    "apps/web/src/app/admin/dashboard/page.tsx",
    "apps/web/src/app/admin/users/page.tsx",
    "apps/web/src/app/admin/memberships/page.tsx",
    "apps/web/src/app/admin/deposits/page.tsx",
    "apps/web/src/app/admin/address-pools/page.tsx",
    "apps/web/src/app/admin/templates/page.tsx",
    "apps/web/src/app/admin/strategies/page.tsx",
    "apps/web/src/app/admin/sweeps/page.tsx",
    "apps/web/src/app/admin/audit/page.tsx",
    "apps/web/src/app/admin/system/page.tsx",
  ];

  for (const page of routePages) {
    assert.doesNotMatch(read(page), /<main[\s>]/, `${page} should rely on shared shell layout`);
  }

  for (const href of [
    "/app/dashboard",
    "/app/exchange",
    "/app/strategies",
    "/app/strategies/new",
    "/app/orders",
    "/app/analytics",
    "/app/billing",
    "/app/telegram",
    "/app/security",
    "/app/help",
    "/admin/dashboard",
    "/admin/users",
    "/admin/memberships",
    "/admin/deposits",
    "/admin/address-pools",
    "/admin/templates",
    "/admin/strategies",
    "/admin/sweeps",
    "/admin/audit",
    "/admin/system",
  ]) {
    assert.match(mockData, new RegExp(escapePattern(href)));
  }

  for (const legacyHref of ["/app/membership", "/app/notifications", "/admin/billing"]) {
    assert.doesNotMatch(mockData, new RegExp(escapePattern(legacyHref)));
  }

  assert.match(helpPage, /normalizeHelpArticle/);
  assert.match(helpPage, /notFound\(/);
  assert.match(helpSlugPage, /getHelpArticle/);
  assert.match(helpSlugPage, /notFound\(/);
  assert.match(helpSlugPage, /shell--public/);
  assert.match(helpSlugPage, /\/app\/help\?article=\$\{article\.slug\}/);

  const serverApi = read("apps/web/src/lib/api/server.ts");
  assert.match(serverApi, /getUserDashboardSnapshot/);
  assert.match(serverApi, /getAdminDashboardSnapshot/);
  assert.match(serverApi, /server-only|"use server"/);
});

test("web shells expose persistent language and theme preferences", () => {
  const requiredFiles = [
    "apps/web/src/lib/ui/preferences.ts",
    "apps/web/src/components/shell/shell-preferences.tsx",
  ];

  for (const file of requiredFiles) {
    assert.ok(fs.existsSync(file), `${file} should exist`);
  }

  const rootLayout = read("apps/web/src/app/layout.tsx");
  const publicShell = read("apps/web/src/components/shell/public-shell.tsx");
  const userShell = read("apps/web/src/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/src/components/shell/admin-shell.tsx");

  assert.doesNotMatch(rootLayout, /<html lang="en">/, "root layout should not hardcode english once i18n preferences exist");
  assert.match(rootLayout, /ui_lang|ui_theme|data-theme|cookies\(/, "root layout should read persisted ui preferences");
  assert.match(publicShell, /ShellPreferences|language|theme|语言|主题/i, "public shell should expose preference controls");
  assert.match(userShell, /ShellPreferences|language|theme|语言|主题/i, "user shell should expose preference controls");
  assert.match(adminShell, /ShellPreferences|language|theme|语言|主题/i, "admin shell should expose preference controls");
});

test("shared shell visual system follows a professional trading-console contract", () => {
  const globalsCss = read("apps/web/src/styles/globals.css");
  const rootLayout = read("apps/web/src/app/layout.tsx");
  const publicShell = read("apps/web/src/components/shell/public-shell.tsx");
  const userShell = read("apps/web/src/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/src/components/shell/admin-shell.tsx");
  const section = read("apps/web/src/components/shell/app-shell-section.tsx");
  const card = read("apps/web/src/components/ui/card.tsx");
  const form = read("apps/web/src/components/ui/form.tsx");
  const banner = read("apps/web/src/components/ui/status-banner.tsx");
  const table = read("apps/web/src/components/ui/table.tsx");
  const chip = read("apps/web/src/components/ui/chip.tsx");
  const dialog = read("apps/web/src/components/ui/dialog.tsx");
  const tabs = read("apps/web/src/components/ui/tabs.tsx");

  assert.match(globalsCss, /--mono-font|IBM Plex Mono/i, "global styles should define mono typography for prices, pnl, and timestamps");
  assert.match(globalsCss, /--panel|--accent-amber|--positive|--negative/i, "global styles should define console-like panel, amber, and red-green tokens");
  assert.match(globalsCss, /grid-template-columns:\s*(240px|15rem|248px|15\.5rem|256px|16rem)\s+minmax\(0,\s*1fr\)/i, "workspace shell should keep a fixed-width sidebar");
  assert.match(globalsCss, /\.shell-topbar[\s\S]*border-bottom/i, "workspace topbar should read like a console header");
  assert.match(globalsCss, /\.metric-strip__item[\s\S]*font-family:\s*var\(--mono-font\)/i, "metrics should use mono numerals");
  assert.match(globalsCss, /\.ui-table[\s\S]*font-variant-numeric:\s*tabular-nums/i, "tables should use tabular numerals");
  assert.match(globalsCss, /\.ui-chip[\s\S]*text-transform:\s*uppercase/i, "chips should keep compact uppercase labels");

  assert.match(rootLayout, /UiLanguageProvider|app-body/, "root layout should continue to own shared shell chrome and language context");
  assert.match(publicShell, /market-strip|console|session|supportLinks/i, "public shell should expose compact console metadata");
  assert.match(userShell, /console|workspace|risk|quickStats/i, "user shell should expose compact console stats");
  assert.match(adminShell, /console|operations|risk|quickStats/i, "admin shell should expose compact console stats");
  assert.match(section, /app-section__header|app-section__content|app-section/i, "app sections should keep shared console framing");
  assert.match(card, /ui-card__eyebrow|ui-card__header|ui-card/i, "card primitive should support dense console framing");
  assert.match(form, /ui-field__meta|ui-input|button/i, "form primitive should support dense field metadata");
  assert.match(banner, /status-banner__meta|status-banner__actions|status-banner/i, "status banner should expose compact metadata and actions");
  assert.match(table, /ui-table__scroller|ui-table|table-wrap/i, "table primitive should support a dedicated scroller wrapper");
  assert.match(chip, /UiLanguageProvider|useUiCopy|ui-chip/i, "shared UI primitives should host a reusable language mapping helper");
  assert.match(dialog, /ui-dialog__header|ui-dialog__body|ui-dialog/i, "dialog primitive should keep structured console framing");
  assert.match(tabs, /ui-tab__meta|ui-tab|ui-tabs/i, "tabs should support compact meta-friendly triggers");
});

test("shared status and table copy localizes instead of leaking fixed english labels", () => {
  const rootLayout = read("apps/web/src/app/layout.tsx");
  const banner = read("apps/web/src/components/ui/status-banner.tsx");
  const dialog = read("apps/web/src/components/ui/dialog.tsx");
  const table = read("apps/web/src/components/ui/table.tsx");
  const chip = read("apps/web/src/components/ui/chip.tsx");

  assert.match(rootLayout, /UiLanguageProvider/, "root layout should provide shared UI language context");
  assert.match(chip, /createContext|pickText|UiLanguageProvider|useUiCopy/, "shared UI language helper should live in shared UI scope");
  assert.match(banner, /pickText|useUiCopy|lang/i, "status banner labels should use language-aware copy");
  assert.match(dialog, /pickText|useUiCopy|lang/i, "dialog labels should use language-aware copy");
  assert.match(table, /pickText|useUiCopy|lang/i, "table empty-state copy should use language-aware copy");
  assert.doesNotMatch(banner, />\s*Heads up\s*</, "dialog/banner tone labels must not be fixed english");
  assert.doesNotMatch(dialog, />\s*Warning\s*</, "dialog tone labels must not be fixed english");
  assert.doesNotMatch(table, /No records available\./, "table empty-state copy must not be fixed english");
});

test("shell and help helpers enforce route behavior", async () => {
  const { isNavHrefActive } = await import("../../apps/web/src/components/shell/path-utils.ts");
  const { isValidHelpArticle, normalizeHelpArticle } = await import("../../apps/web/src/lib/api/help-articles.ts");

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
  const serverApi = read("apps/web/src/lib/api/server.ts");
  const dashboardPage = read("apps/web/src/app/app/dashboard/page.tsx");
  const securityPage = read("apps/web/src/app/app/security/page.tsx");
  const ordersPage = read("apps/web/src/app/app/orders/page.tsx");
  const exchangePage = read("apps/web/src/app/app/exchange/page.tsx");
  const appLayout = read("apps/web/src/app/app/layout.tsx");

  assert.doesNotMatch(serverApi, /import\("\.\/user-product-state"\)/, "user shell snapshot should not be derived from user-product-state");
  assert.doesNotMatch(serverApi, /state\.strategies/, "user shell quick stats should not depend on fabricated strategy state");
  assert.doesNotMatch(dashboardPage, /getCurrentUserProductState/, "dashboard must fetch backend truth directly");
  assert.doesNotMatch(securityPage, /getCurrentUserProductState/, "security center must not read local product state for core posture");
  assert.doesNotMatch(ordersPage, /ORD-\$\{/, "orders page must not fabricate display order ids from strategy ids");
  assert.match(exchangePage, /Validation details|Permissions OK|Hedge mode OK/i, "exchange page should show detailed validation failures");
  assert.match(appLayout, /notifications|show_expiry_popup|Expiry|grace/i, "user app layout should own expiry popup or notification banner wiring");
  assert.match(appLayout, /aria-modal="true"|modal/i, "expiry reminder should render through the dialog modal shell");
});


test("public landing and auth pages localize primary copy instead of single-language rendering", () => {
  const homePage = read("apps/web/src/app/(public)/page.tsx");
  const loginPage = read("apps/web/src/app/(public)/login/page.tsx");
  const registerPage = read("apps/web/src/app/(public)/register/page.tsx");

  assert.match(homePage, /pickText|resolveUiLanguage|cookies\(/);
  assert.match(loginPage, /pickText|resolveUiLanguage|cookies\(/);
  assert.match(registerPage, /pickText|resolveUiLanguage|cookies\(/);
  assert.match(homePage, /创建账号|Create account/);
  assert.match(loginPage, /重置密码|Reset password/);
  assert.match(registerPage, /查看计费说明|Billing help/);
});

test("strategy save route does not auto-pause running strategies", () => {
  const strategyRoute = read("apps/web/src/app/api/user/strategies/[id]/route.ts");

  assert.match(strategyRoute, /Strategy must be paused before editing and saving changes\./, "save flow should surface an explicit pause-first message");
});


test("strategy workspace exposes batch actions, user templates, and real multi-level payload plumbing", () => {
  const strategiesPage = read("apps/web/src/app/app/strategies/page.tsx");
  const newStrategyPage = read("apps/web/src/app/app/strategies/new/page.tsx");
  const detailPage = read("apps/web/src/app/app/strategies/[id]/page.tsx");
  const createRoute = read("apps/web/src/app/api/user/strategies/create/route.ts");
  const detailRoute = read("apps/web/src/app/api/user/strategies/[id]/route.ts");
  const templatesRoute = read("apps/web/src/app/api/user/strategies/templates/route.ts");
  const backendStrategiesRoute = read("apps/api-server/src/routes/strategies.rs");

  assert.match(strategiesPage, /\/api\/user\/strategies\/batch/);
  assert.match(strategiesPage, /stop-all/);
  assert.match(strategiesPage, /Start selected|Pause selected|Delete selected|Stop all/i);
  assert.match(strategiesPage, /Start filtered|Pause filtered|Delete filtered/i);

  assert.match(newStrategyPage, /overall/i, "new strategy form should expose overall TP\/SL inputs");
  assert.match(newStrategyPage, /Apply template/i, "new strategy form should expose template application");
  assert.match(newStrategyPage, /Search symbols|symbolQuery|Symbol search/i, "new strategy page should expose fuzzy symbol search");
  assert.match(newStrategyPage, /Amount mode|Quote amount|Base asset quantity/i, "new strategy page should expose quote-vs-asset amount controls");
  assert.match(newStrategyPage, /Batch spacing|Batch take profit|Reference price|Grid count/i, "new strategy page should expose batch grid editing controls");
  assert.match(detailPage, /Search symbols|symbolQuery|Symbol search/i, "strategy detail should expose fuzzy symbol search");
  assert.match(detailPage, /Amount mode|Quote amount|Base asset quantity/i, "strategy detail should keep the amount mode controls after draft creation");
  assert.match(detailPage, /Batch spacing|Batch take profit|Reference price|Grid count/i, "strategy detail should expose batch edit controls instead of JSON-only editing");
  assert.match(newStrategyPage, /exchange\/binance\/symbols\/search|symbols\/search/);
  assert.match(detailPage, /exchange\/binance\/symbols\/search|symbols\/search/);
  assert.match(newStrategyPage, /levelsJson|levels_json/, "new strategy form should submit real multi-level data");

  assert.match(createRoute, /levels_json/);
  assert.match(createRoute, /amountMode|quoteAmount|baseQuantity|referencePrice|gridSpacingPercent|gridCount|batchTakeProfit|batchTrailing/i);
  assert.match(createRoute, /JSON\.parse/);
  assert.doesNotMatch(createRoute, /levels:\s*\[\s*\{[\s\S]*\{[\s\S]*\{[\s\S]*\]/, "create route must not hardcode a fixed three-level payload");

  assert.match(detailPage, /Pause first to edit|paused before editing|pause before edit/i);
  assert.match(detailPage, /Runtime events/i);
  assert.match(detailPage, /Running strategy parameters cannot be hot-modified|Trading-critical warning/i);
  assert.match(detailPage, /overall/i);
  assert.match(detailPage, /levelsJson|levels_json/);
  assert.match(detailRoute, /levels_json/);
  assert.match(detailRoute, /amountMode|quoteAmount|baseQuantity|referencePrice|gridSpacingPercent|gridCount|batchTakeProfit|batchTrailing/i);
  assert.match(detailRoute, /overallTakeProfit|overallStopLoss/);

  assert.match(templatesRoute, /\/strategies\/templates/);
  assert.match(templatesRoute, /\/apply/);
  assert.match(backendStrategiesRoute, /\/strategies\/templates/);
  assert.match(backendStrategiesRoute, /\/strategies\/templates\/\{template_id\}\/apply/);
});


test("auth proxy routes only expose verification cookies through test capture paths", () => {
  const registerRoute = read("apps/web/src/app/api/auth/register/route.ts");
  const resetRoute = read("apps/web/src/app/api/auth/password-reset/route.ts");

  assert.match(registerRoute, /const secureCookie = process\.env\.NODE_ENV === \"production\"/);
  assert.match(resetRoute, /const secureCookie = process\.env\.NODE_ENV === \"production\"/);
  assert.match(registerRoute, /if \(registerResponse\.verification_code\)/);
  assert.match(resetRoute, /if \(responseBody\.reset_code\)/);
  assert.match(registerRoute, /secure: secureCookie/);
  assert.match(resetRoute, /secure: secureCookie/);
});


test("strategy workspace exposes template application and batch lifecycle controls", () => {
  const strategiesPage = read("apps/web/src/app/app/strategies/page.tsx");
  const newStrategyPage = read("apps/web/src/app/app/strategies/new/page.tsx");
  const strategyApi = read("apps/api-server/src/routes/strategies.rs");

  assert.match(strategiesPage, /Start selected/);
  assert.match(strategiesPage, /Pause selected/);
  assert.match(strategiesPage, /Delete selected/);
  assert.match(strategiesPage, /Stop all/);
  assert.match(newStrategyPage, /Strategy templates/);
  assert.match(newStrategyPage, /overallTakeProfit|Overall take profit/);
  assert.match(newStrategyPage, /levels_json|Grid levels JSON/);
  assert.match(strategyApi, /strategies\/templates/);
});


test("billing page surfaces address assignment, queue, and lock timing semantics", () => {
  const billingPage = read("apps/web/src/app/app/billing/page.tsx");
  const billingRoute = read("apps/web/src/app/api/user/billing/route.ts");

  assert.match(billingPage, /Assigned address|Queue position|Address lock expires/i);
  assert.match(billingPage, /queue_position|address/i);
  assert.match(billingRoute, /expires_at|queue_position/i);
  assert.match(billingRoute, /address/i);
});

test("telegram page distinguishes delivered, failed, web-only, and not-bound outcomes", () => {
  const telegramPage = read("apps/web/src/app/app/telegram/page.tsx");

  assert.match(telegramPage, /Delivered/);
  assert.match(telegramPage, /Failed/);
  assert.match(telegramPage, /Web only/i);
  assert.match(telegramPage, /Not bound/i);
});

test("user workflow guides explain batch editing, restart rules, and billing remediation", () => {
  const createGuide = read("docs/user-guide/create-grid-strategy.md");
  const manageGuide = read("docs/user-guide/manage-strategy.md");
  const billingGuide = read("docs/user-guide/membership-and-payment.md");

  assert.match(createGuide, /Amount mode|Quote amount|Base asset quantity/i);
  assert.match(createGuide, /Batch spacing|Batch take profit|Advanced JSON/i);
  assert.match(manageGuide, /Start selected|Pause selected|Delete selected|Stop all/i);
  assert.match(manageGuide, /Pause, save, re-run pre-flight, then restart/i);
  assert.match(billingGuide, /Assigned address|Queue position|address lock/i);
  assert.match(billingGuide, /manual review|grace period|exact amount/i);
});
