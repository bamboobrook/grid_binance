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

function escapePattern(input) {
  return input.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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
    gettingStarted.body.includes("- `/app/dashboard` for account overview and renewal reminders"),
    "repository bullet content should stay available to the in-app help renderer",
  );
  assert.match(helpPage, /\/app\/help\?article=\$\{item\.slug\}/);
  assert.doesNotMatch(helpPage, /href=\{`\/help\/\$\{item\.slug\}`\}/);
  assert.match(helpPage, /blocks\.map\(/);
});

test("smoke checks mention the commercial runtime path", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /deploy\/docker\/docker-compose\.yml/);
  assert.match(script, /http:\/\/localhost:8080\/app\/dashboard/);
  assert.match(script, /http:\/\/localhost:8080\/admin\/dashboard/);
  assert.match(script, /http:\/\/localhost:8080\/help\/getting-started/);
});

test("docker compose guide lists the actual commercial stack services", () => {
  const guide = fs.readFileSync("docs/deployment/docker-compose.md", "utf8");

  for (const service of [
    "postgres",
    "redis",
    "api-server",
    "trading-engine",
    "scheduler",
    "market-data-gateway",
    "billing-chain-listener",
    "web",
    "nginx",
    "prometheus",
  ]) {
    assert.match(guide, new RegExp("`" + escapePattern(service) + "`"));
  }

  assert.match(guide, /5 Rust services/i);
});
