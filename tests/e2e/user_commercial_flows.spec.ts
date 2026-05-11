const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const TELEGRAM_BOT_BIND_SECRET =
  process.env.TELEGRAM_BOT_BIND_SECRET ?? "grid-binance-dev-telegram-bot-bind-secret";

import { expect, test } from "@playwright/test";

import { registerViaPage, uniqueEmail } from "./support/auth";

test.describe("user commercial", () => {
  test("landing page exposes current public entry copy and actions", async ({ page }) => {
    await page.goto("/");

    await expect(
      page.getByRole("heading", { name: /The Ultimate Grid/i }),
    ).toBeVisible();
    await expect(page.getByRole("link", { name: /Start trading for free/i }).first()).toBeVisible();
    await expect(page.getByRole("link", { name: /View Demo Account/i }).first()).toBeVisible();
    await expect(page.getByText("Advanced Grid & DCA Bots", { exact: false })).toBeVisible();
    await expect(page.getByText("Risk Management", { exact: false })).toBeVisible();
  });

  test("workspace covers the current user lifecycle flows", async ({ page, request }) => {
    const email = uniqueEmail("commercial");
    const password = "pass1234";

    await registerViaPage(page, email, password);

    await expect(page).toHaveURL(/\/app\/dashboard$/);
    await expect(page.getByRole("heading", { name: "Trading Cockpit" })).toBeVisible();
    await expect(page.getByText("Realized PnL", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Unrealized PnL", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Net PnL", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Running Bots", { exact: false }).first()).toBeVisible();

    await page.goto("/app/analytics");
    await expect(page.getByRole("heading", { name: "Analytics", exact: true })).toBeVisible();
    await expect(page.getByText("Fees paid", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Funding total", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Strategy statistics", { exact: false }).first()).toBeVisible();

    await page.goto("/app/exchange");
    await page.getByLabel("Binance API key").fill("BNB1-USER-KEY-7890");
    await page.getByLabel("Binance API secret").fill("super-secret-binance-key");
    await page.getByRole("button", { name: "Save credentials" }).click();
    await expect(page).toHaveURL(/\/app\/exchange(?:\?exchange=credentials-saved)?$/);
    expect(page.url()).not.toContain("apiSecret");
    expect(page.url()).not.toContain("super-secret");
    await expect(page.getByText("Credentials saved", { exact: false })).toBeVisible();
    await expect(page.getByText("Validation details", { exact: false })).toBeVisible();

    await page.goto("/app/billing");
    await page.getByLabel("Plan").selectOption("yearly");
    await page.getByLabel("Chain").selectOption("BSC");
    await page.getByLabel("Token").selectOption("USDT");
    await page.getByRole("button", { name: "Create payment order" }).click();
    await expect(page).toHaveURL(/\/app\/billing(?:\?notice=.*)?$/);
    await expect(page.getByText("Awaiting exact transfer", { exact: false })).toBeVisible();
    await expect(page.getByText("Assigned address", { exact: false })).toHaveCount(1);
    await expect(page.getByText("Address lock expires", { exact: false }).first()).toBeVisible();

    await page.goto("/app/strategies/new");
    await expect(page.getByLabel("Strategy Name")).toBeVisible();
    await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
    await page.getByLabel("Strategy Name").fill("ETH Swing Builder");
    await page.getByLabel("Market Type").selectOption("usd-m");
    await page.getByLabel("Strategy Mode").selectOption("short");
    await page.locator('select[name="amountMode"]').selectOption("quote");
    await page.getByLabel("Per-grid Quote Amount (USDT)").fill("1200");
    await page.getByLabel("Reference Price").fill("100");
    await page.getByLabel("Grid Count").fill("4");
    await page.getByLabel("Batch Spacing (%)").fill("1.5");
    await page.getByLabel("Grid Take Profit (%)").first().fill("2.2");
    await page.getByRole("button", { name: "Create Bot" }).click();
    await expect(page).toHaveURL(/\/app\/strategies\/strategy-\d+(?:\?notice=draft-saved)?$/);
    await expect(page.getByText("Strategy Detail", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Run Pre-flight" }).click();
    await expect(page.getByText("Pre-flight Failed", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Start Strategy" }).click();
    await expect(page.getByText("Start Failed", { exact: false })).toBeVisible();
    await expect(page.getByText("Membership Status", { exact: false }).first()).toBeVisible();

    await page.goto("/app/orders");
    await expect(page.getByRole("heading", { name: "Orders & History" })).toBeVisible();
    await expect(page.getByText("Strategy orders", { exact: false })).toBeVisible();
    await expect(page.getByText("Fill history", { exact: false })).toBeVisible();

    await page.goto("/app/telegram");
    await expect(page.getByRole("heading", { name: "Telegram Notifications" })).toBeVisible();
    await page.getByRole("button", { name: "Generate bind code" }).click();
    await expect(page).toHaveURL(/\/app\/telegram\?notice=bind-code-issued/);
    await expect(page.getByText("Bind code issued", { exact: false })).toBeVisible();
    await expect(page.getByRole("link", { name: "Open Telegram Bot" }).first()).toBeVisible();
    const bindCode =
      (await page.locator("strong").allTextContents()).find((value) => value.startsWith("tg-bind-")) ?? "";
    expect(bindCode).toMatch(/^tg-bind-[a-f0-9]+$/);
    const botBind = await request.post(`${AUTH_API_BASE_URL}/telegram/bot/bind`, {
      data: {
        code: bindCode,
        telegram_user_id: `tg-${Date.now()}`,
        chat_id: `chat-${Date.now()}`,
        username: "commercial_user",
      },
      headers: {
        "content-type": "application/json",
        "x-telegram-bot-secret": TELEGRAM_BOT_BIND_SECRET,
      },
    });
    expect(botBind.status(), await botBind.text()).toBe(200);
    await page.goto("/app/telegram");
    await expect(page.getByText("Telegram bound at", { exact: false })).toBeVisible();

    await page.goto("/app/security");
    await page.getByLabel("Current password").fill("pass1234");
    await page.getByLabel("New password").fill("pass12345");
    await page.getByRole("button", { name: "Update password" }).click();
    await expect(page).toHaveURL(/\/login\?security=password-updated$/);
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill("pass12345");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page).toHaveURL(/\/app\/dashboard$/);

    await page.goto("/app/help");
    await expect(page.getByRole("heading", { name: "Help Center", exact: true })).toBeVisible();
    await page.getByRole("link", { name: "Create Grid Strategy" }).click();
    await expect(page).toHaveURL(/\/app\/help\?article=create-grid-strategy$/);
    await expect(page.getByRole("heading", { name: "Create Grid Strategy", exact: true })).toBeVisible();
  });

  test("telegram rebind keeps the fresh bind command visible until the new account completes binding", async ({
    page,
    request,
  }) => {
    const email = uniqueEmail("telegram-rebind");
    const password = "pass1234";

    await registerViaPage(page, email, password);
    await page.goto("/app/telegram");
    await expect(page.getByRole("heading", { name: "Telegram Notifications" })).toBeVisible();

    await page.getByRole("button", { name: "Generate bind code" }).click();
    await expect(page).toHaveURL(/\/app\/telegram\?notice=bind-code-issued/);
    const firstBindCode =
      (await page.locator("strong").allTextContents()).find((value) => value.startsWith("tg-bind-")) ?? "";
    expect(firstBindCode).toMatch(/^tg-bind-[a-f0-9]+$/);

    const firstBind = await request.post(`${AUTH_API_BASE_URL}/telegram/bot/bind`, {
      data: {
        code: firstBindCode,
        telegram_user_id: `tg-initial-${Date.now()}`,
        chat_id: `chat-initial-${Date.now()}`,
        username: "initial_account",
      },
      headers: {
        "content-type": "application/json",
        "x-telegram-bot-secret": TELEGRAM_BOT_BIND_SECRET,
      },
    });
    expect(firstBind.status(), await firstBind.text()).toBe(200);

    await page.goto("/app/telegram");
    await expect(page.getByText("Telegram bound at", { exact: false })).toBeVisible();

    await page.getByRole("button", { name: "Rebind Telegram account" }).click();
    await expect(page).toHaveURL(/\/app\/telegram\?notice=bind-code-issued/);
    await expect(page.getByText("Bind code issued", { exact: false })).toBeVisible();

    const secondBindCode =
      (await page.locator("strong").allTextContents()).find((value) => value.startsWith("tg-bind-")) ?? "";
    expect(secondBindCode).toMatch(/^tg-bind-[a-f0-9]+$/);
    expect(secondBindCode).not.toBe(firstBindCode);
    await expect(page.getByText(`/bind ${secondBindCode}`, { exact: false })).toBeVisible();
    await expect(page.getByText("Generate new bind code", { exact: false })).toBeVisible();
  });

  test("user commercial help center renders repository docs", async ({ page }) => {
    await page.goto("/help/telegram-notifications");

    await expect(page.getByRole("heading", { name: "Telegram Notifications" }).first()).toBeVisible();
    await expect(
      page.getByText(
        "Open `/app/telegram` after sign-in. This is the canonical app route for Telegram binding and delivery status.",
        { exact: false },
      ),
    ).toBeVisible();
  });
});
