import { expect, test } from "@playwright/test";

import { registerViaPage, uniqueEmail } from "./support/auth";

test.describe("user commercial", () => {
  test("landing page exposes pricing and risk copy", async ({ page }) => {
    await page.goto("/");

    await expect(
      page.getByRole("heading", { name: "Commercial Grid Trading For Binance Users" }),
    ).toBeVisible();
    await expect(page.getByRole("heading", { name: "Monthly" })).toBeVisible();
    await expect(page.getByText("20 USD equivalent", { exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Quarterly" })).toBeVisible();
    await expect(page.getByText("18 USD equivalent per month", { exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Yearly" })).toBeVisible();
    await expect(page.getByText("15 USD equivalent per month", { exact: true })).toBeVisible();
    await expect(page.getByText("Do not enable withdrawal permission", { exact: false })).toBeVisible();
    await expect(page.getByText("Payment amount must match exactly", { exact: false })).toBeVisible();
    await expect(page.getByText("Trailing take profit uses taker execution", { exact: false })).toBeVisible();
  });

  test("workspace covers exchange billing strategy orders telegram security and help flows", async ({
    page,
  }) => {
    const email = uniqueEmail("commercial");
    const password = "pass1234";

    await registerViaPage(page, email, password);

    await expect(page).toHaveURL(/\/app\/dashboard$/);
    await expect(page.getByRole("heading", { name: "User Dashboard" })).toBeVisible();
    await expect(page.getByText("Action queue", { exact: true })).toBeVisible();
    await expect(page.getByText("Complete exchange connection test", { exact: true })).toBeVisible();
    await expect(page.getByText("Resolve error-paused strategy", { exact: true })).toBeVisible();

    await page.goto("/app/exchange");
    await page.getByLabel("Binance API key").fill("BNB1-USER-KEY-7890");
    await page.getByLabel("Binance API secret").fill("super-secret-binance-key");
    await page.getByRole("button", { name: "Save credentials" }).click();
    await expect(page.getByText("Credentials saved", { exact: true })).toBeVisible();
    await expect(page.getByText("BNB1••••7890", { exact: false })).toBeVisible();
    await page.getByRole("link", { name: "Run connection test" }).click();
    await expect(page.getByText("Connection test passed", { exact: true })).toBeVisible();
    await expect(page.getByText("Spot, USDⓈ-M, and COIN-M permissions verified", { exact: false })).toBeVisible();

    await page.goto("/app/billing");
    await page.getByLabel("Plan").selectOption("yearly");
    await page.getByLabel("Chain").selectOption("bsc");
    await page.getByLabel("Token").selectOption("usdt");
    await page.getByRole("button", { name: "Create payment order" }).click();
    await expect(page.getByRole("alert").filter({ hasText: "Awaiting exact transfer" })).toBeVisible();
    await expect(page.getByText("Send exactly 180.00 USDT on BSC", { exact: false })).toBeVisible();
    await expect(page.getByRole("alert").filter({ hasText: "Overpayment, underpayment, or wrong token will require manual review" })).toBeVisible();

    await page.goto("/app/strategies/new");
    await page.getByLabel("Strategy name").fill("BTC Recovery Ladder");
    await page.getByLabel("Symbol").fill("BTCUSDT");
    await page.getByLabel("Market type").selectOption("spot");
    await page.getByRole("button", { name: "Save draft" }).click();
    await expect(page).toHaveURL(/\/app\/strategies\/grid-btc\?draft=1/);
    await expect(page.getByText("Draft saved", { exact: true })).toBeVisible();
    await page.getByLabel("Trailing take profit (%)").fill("0.8");
    await page.getByRole("button", { name: "Save edits" }).click();
    await expect(page.getByText("Edits saved", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Run pre-flight" }).click();
    await expect(page.getByText("Pre-flight passed", { exact: true })).toBeVisible();
    await expect(page.getByText("Exchange filters, balance, and hedge-mode checks passed", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Start strategy" }).click();
    await expect(page.getByText("Strategy started", { exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Running strategy parameters cannot be hot-modified" })).toBeVisible();

    await page.goto("/app/orders");
    await expect(page.getByRole("heading", { name: "Orders & History" })).toBeVisible();
    await expect(page.getByText("Strategy orders", { exact: true })).toBeVisible();
    await expect(page.getByText("Fill history", { exact: true })).toBeVisible();
    await expect(page.getByText("Trailing TP exit", { exact: true })).toBeVisible();

    await page.goto("/app/telegram");
    await page.getByRole("button", { name: "Generate bind code" }).click();
    await expect(page.getByText("Bind code", { exact: true })).toBeVisible();
    await expect(page.getByText("/start GB-", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "I sent the code to the bot" }).click();
    await expect(page.getByText("Telegram bound", { exact: true })).toBeVisible();

    await page.goto("/app/security");
    await page.getByLabel("New password").fill("pass12345");
    await page.getByRole("button", { name: "Update password" }).click();
    await expect(page.getByText("Password updated", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Enable TOTP" }).click();
    await expect(page.getByText("TOTP enabled", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Revoke other sessions" }).click();
    await expect(page.getByText("Other sessions revoked", { exact: true })).toBeVisible();

    await page.goto("/app/help");
    await expect(page.getByRole("heading", { name: "Help Center" })).toBeVisible();
    await page.getByRole("link", { name: "Expiry reminder guide" }).click();
    await expect(page).toHaveURL(/\/help\/expiry-reminder$/);
    await expect(page.getByRole("heading", { name: "Expiry Reminder" })).toBeVisible();
    await expect(page.getByText("Expiry And Grace Period", { exact: true })).toBeVisible();
    await expect(page.getByText("Existing running strategies may continue for 48 hours", { exact: false })).toBeVisible();
  });
});
