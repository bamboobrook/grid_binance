import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const requiredUserGuides = [
  "getting-started.md",
  "binance-api-setup.md",
  "membership-and-payment.md",
  "create-grid-strategy.md",
  "manage-strategy.md",
  "security-center.md",
  "telegram-notifications.md",
  "troubleshooting.md",
];

const requiredAdminGuides = [
  "address-pool-management.md",
  "membership-operations.md",
  "template-management.md",
  "abnormal-order-handling.md",
  "system-config-and-audit.md",
];

const requiredDeploymentGuides = [
  "docker-compose.md",
  "env-and-secrets.md",
  "backup-and-restore.md",
];

function readUserGuide(file) {
  return fs.readFileSync(path.join("docs", "user-guide", file), "utf8");
}
function readText(file) {
  return fs.readFileSync(file, "utf8");
}
function listComposeServices(composeText) {
  const servicesSection = composeText.match(/^services:\n([\s\S]*?)^volumes:\n/m);
  assert.ok(servicesSection, "compose should contain services and volumes sections");

  return [...servicesSection[1].matchAll(/^  ([a-z0-9-]+):$/gm)].map((match) => match[1]);
}
function listDocumentedComposeServices(guideText) {
  const includedServicesSection = guideText.match(/## Included Services\n\n([\s\S]*?)\n## /);
  assert.ok(includedServicesSection, "docker compose guide should contain an Included Services section");

  return [...includedServicesSection[1].matchAll(/^- `([^`]+)` /gm)].map((match) => match[1]);
}

test("commercial guide set matches the March 31 design doc", () => {
  for (const file of requiredUserGuides) {
    assert.ok(
      fs.existsSync(path.join("docs", "user-guide", file)),
      `docs/user-guide/${file} should exist`,
    );
  }

  for (const file of requiredAdminGuides) {
    assert.ok(
      fs.existsSync(path.join("docs", "admin-guide", file)),
      `docs/admin-guide/${file} should exist`,
    );
  }

  for (const file of requiredDeploymentGuides) {
    assert.ok(
      fs.existsSync(path.join("docs", "deployment", file)),
      `docs/deployment/${file} should exist`,
    );
  }
});

test("help center stays sourced from repository user guides", async () => {
  const { VALID_HELP_ARTICLES, getHelpArticle } = await import(
    "../../apps/web/src/lib/api/help-articles.ts"
  );
  const helpPage = fs.readFileSync("apps/web/src/app/app/help/page.tsx", "utf8");

  for (const file of requiredUserGuides) {
    const slug = file.replace(/\.md$/, "");
    assert.ok(
      VALID_HELP_ARTICLES.includes(slug),
      `help center should expose ${slug}`,
    );
  }

  const gettingStarted = getHelpArticle("getting-started");
  assert.ok(gettingStarted, "getting-started article should be available");
  assert.ok(
    gettingStarted.body.includes("## First Run Path"),
    "repository headings should stay available to the in-app help renderer",
  );
  assert.ok(
    gettingStarted.body.includes("- `/app/orders` for fills, order history, and account activity review"),
    "repository bullet content should stay available to the in-app help renderer",
  );
  assert.ok(
    gettingStarted.body.includes("- `/app/telegram` for Telegram bot binding and notification delivery status"),
    "repository routes should stay aligned with the canonical app shell pages",
  );
  assert.match(helpPage, /\/app\/help\?article=\$\{item\.slug\}/);
  assert.doesNotMatch(helpPage, /href=\{`\/help\/\$\{item\.slug\}`\}/);
  assert.match(helpPage, /blocks\.map\(/);
});

test("user guides stay aligned with canonical app and help routes", () => {
  const gettingStarted = readUserGuide("getting-started.md");
  const binanceApiSetup = readUserGuide("binance-api-setup.md");
  const createGridStrategy = readUserGuide("create-grid-strategy.md");
  const manageStrategy = readUserGuide("manage-strategy.md");
  const telegramNotifications = readUserGuide("telegram-notifications.md");

  assert.match(gettingStarted, /`\/app\/help\?article=getting-started`/);
  assert.match(gettingStarted, /`\/app\/help\?article=<slug>`/);
  assert.match(gettingStarted, /`\/help\/getting-started`/);
  assert.match(gettingStarted, /`\/help\/<slug>`/);
  assert.match(gettingStarted, /public help route/i);
  assert.match(gettingStarted, /in-app help/i);

  assert.match(binanceApiSetup, /`\/app\/exchange`/);
  assert.match(createGridStrategy, /`\/app\/strategies\/new`/);
  assert.match(manageStrategy, /`\/app\/strategies\/:id`/);
  assert.match(telegramNotifications, /`\/app\/telegram`/);
});

test("smoke checks mention the commercial runtime path", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /deploy\/docker\/docker-compose\.yml/);
  assert.match(script, /http:\/\/localhost:8080\/app\/dashboard/);
  assert.match(script, /http:\/\/localhost:8080\/admin\/dashboard/);
  assert.match(script, /http:\/\/localhost:8080\/help\/getting-started/);
});

test("docker compose guide lists the actual commercial stack services", () => {
  const compose = readText("deploy/docker/docker-compose.yml");
  const guide = readText("docs/deployment/docker-compose.md");
  const actualServices = listComposeServices(compose);
  const documentedServices = listDocumentedComposeServices(guide);
  const rustServiceCount = actualServices.filter((service) =>
    ["api-server", "trading-engine", "scheduler", "market-data-gateway", "billing-chain-listener"].includes(service),
  ).length;

  assert.deepEqual(
    documentedServices,
    actualServices,
    "docker compose guide should document the exact compose service list in order",
  );
  assert.match(
    guide,
    new RegExp(`${actualServices.length} services in total, including ${rustServiceCount} Rust services`, "i"),
  );
});

test("public help surface includes expiry reminder from repository-backed article content", async () => {
  const { VALID_HELP_ARTICLES, getHelpArticle } = await import(
    "../../apps/web/src/lib/api/help-articles.ts"
  );
  const publicHelpPage = readText("apps/web/src/app/help/[slug]/page.tsx");
  const publicLanding = readText("apps/web/src/app/(public)/page.tsx");
  const publicLogin = readText("apps/web/src/app/(public)/login/page.tsx");
  const publicRegister = readText("apps/web/src/app/(public)/register/page.tsx");
  const expiryReminderDoc = readText("docs/user-guide/expiry-reminder.md");

  assert.ok(
    VALID_HELP_ARTICLES.includes("expiry-reminder"),
    "public help articles should expose expiry-reminder",
  );
  assert.match(expiryReminderDoc, /^# Expiry Reminder$/m);
  assert.match(expiryReminderDoc, /^## Expiry And Grace Period$/m);
  assert.match(expiryReminderDoc, /48-hour grace period/i);
  assert.match(expiryReminderDoc, /auto-pauses running strategies/i);
  assert.match(expiryReminderDoc, /exact chain, token, and amount shown on the billing page/i);

  const article = getHelpArticle("expiry-reminder");
  assert.ok(article, "expiry-reminder article should be available");
  assert.equal(article.slug, "expiry-reminder");
  assert.equal(article.title, "Expiry Reminder");
  assert.equal(article.summary, "Understand the 48-hour grace period and what happens when membership renewal is delayed.");
  assert.ok(
    article.body.includes("## Expiry And Grace Period"),
    "expiry reminder article should preserve repository headings for public rendering",
  );
  assert.ok(
    article.body.includes("Existing running strategies may continue for 48 hours after membership expiry, but new starts stay blocked until renewal is confirmed."),
    "expiry reminder article should preserve grace-period body copy",
  );

  assert.match(publicHelpPage, /StatusBanner description=\{article\.summary\} title=\{article\.title\}/);
  assert.match(publicHelpPage, /href=\{`\/app\/help\?article=\$\{article\.slug\}`\}/);
  assert.match(publicHelpPage, /Open Billing Center/);
  assert.match(publicLanding, /href="\/help\/expiry-reminder"/);
  assert.match(publicLogin, /href="\/help\/expiry-reminder"/);
  assert.match(publicRegister, /href="\/help\/expiry-reminder"/);
});
