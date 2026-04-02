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

  test("workspace covers secure user-product flows with richer stats and lifecycle state", async ({
    page,
  }) => {
    const email = uniqueEmail("commercial");
    const password = "pass1234";

    await registerViaPage(page, email, password);

    await expect(page).toHaveURL(/\/app\/dashboard$/);
    await expect(page.getByRole("heading", { name: "User Dashboard" })).toBeVisible();
    await expect(page.getByText("Total realized PnL", { exact: true })).toBeVisible();
    await expect(page.getByText("Total unrealized PnL", { exact: true })).toBeVisible();
    await expect(page.getByText("Total fees", { exact: true })).toBeVisible();
    await expect(page.getByText("Total funding fees", { exact: true })).toBeVisible();
    await expect(page.getByText("Exchange account activity", { exact: true })).toBeVisible();
    await expect(page.getByText("Membership status", { exact: true })).toBeVisible();
    await expect(page.getByText("Error-paused strategies", { exact: true })).toBeVisible();

    await page.goto("/app/exchange");
    await page.getByLabel("Binance API key").fill("BNB1-USER-KEY-7890");
    await page.getByLabel("Binance API secret").fill("super-secret-binance-key");
    await page.getByRole("button", { name: "Save credentials" }).click();
    await expect(page).toHaveURL(/\/app\/exchange$/);
    expect(page.url()).not.toContain("apiSecret");
    expect(page.url()).not.toContain("super-secret");
    await expect(page.getByText("Credentials saved", { exact: true })).toBeVisible();
    await expect(page.getByText("BNB1••••7890", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Run connection test" }).click();
    await expect(page).toHaveURL(/\/app\/exchange$/);
    await expect(page.getByText("Connection test passed", { exact: true })).toBeVisible();
    await expect(page.getByText("Spot, USDⓈ-M, and COIN-M permissions verified", { exact: false })).toBeVisible();

    await page.goto("/app/billing");
    await page.getByLabel("Plan").selectOption("yearly");
    await page.getByLabel("Chain").selectOption("bsc");
    await page.getByLabel("Token").selectOption("usdt");
    await page.getByRole("button", { name: "Create payment order" }).click();
    await expect(page).toHaveURL(/\/app\/billing$/);
    await expect(page.getByRole("alert").filter({ hasText: "Awaiting exact transfer" })).toBeVisible();
    await expect(page.getByText("Send exactly 180.00 USDT on BSC", { exact: false })).toBeVisible();
    await expect(page.getByRole("alert").filter({ hasText: "Overpayment, underpayment, or wrong token will require manual review" })).toBeVisible();

    await page.goto("/app/strategies/new");
    await page.getByLabel("Strategy name").fill("ETH Swing Builder");
    await page.getByLabel("Symbol").fill("ETHUSDT");
    await page.getByLabel("Market type").selectOption("usd-m");
    await page.getByLabel("Strategy mode").selectOption("short");
    await page.getByRole("button", { name: "Save draft" }).click();
    await expect(page).toHaveURL(/\/app\/strategies\/eth-swing-builder$/);
    await expect(page.getByText("Draft saved", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Start strategy" }).click();
    await expect(page.getByText("Start blocked until pre-flight passes", { exact: true })).toBeVisible();
    await page.getByLabel("Trailing take profit (%)").fill("0.7");
    await page.getByRole("button", { name: "Save edits" }).click();
    await expect(page.getByText("Edits saved", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Run pre-flight" }).click();
    await expect(page.getByText("Pre-flight passed", { exact: true })).toBeVisible();
    await expect(page.getByText("Exchange filters, balance, and hedge-mode checks passed", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Start strategy" }).click();
    await expect(page.getByText("Strategy started", { exact: true })).toBeVisible();
    await expect(page.getByText("Realized PnL", { exact: true })).toBeVisible();
    await expect(page.getByText("Unrealized PnL", { exact: true })).toBeVisible();
    await expect(page.getByText("Funding fees", { exact: true })).toBeVisible();
    await expect(page.getByText("Cost basis", { exact: true })).toBeVisible();
    await expect(page.getByText("Current holdings", { exact: true })).toBeVisible();

    await page.goto("/app/orders");
    await expect(page.getByRole("heading", { name: "Orders & History" })).toBeVisible();
    await expect(page.getByText("Strategy orders", { exact: true })).toBeVisible();
    await expect(page.getByText("Fill history", { exact: true })).toBeVisible();
    await expect(page.getByText("Trailing TP exit", { exact: true })).toBeVisible();

    await page.goto("/app/telegram");
    await page.getByRole("button", { name: "Generate bind code" }).click();
    await expect(page).toHaveURL(/\/app\/telegram$/);
    await expect(page.getByText("Bind code issued", { exact: true })).toBeVisible();
    await expect(page.getByText(/^GB-\d{4}$/)).toBeVisible();
    await page.getByRole("button", { name: "I sent the code to the bot" }).click();
    await expect(page).toHaveURL(/\/app\/telegram$/);
    await expect(page.getByText("Telegram bound", { exact: true })).toBeVisible();

    await page.goto("/app/security");
    await page.getByLabel("Current password").fill("pass1234");
    await page.getByLabel("New password").fill("pass12345");
    await page.getByRole("button", { name: "Update password" }).click();
    await expect(page).toHaveURL(/\/login\?security=password-updated$/);
    await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill("pass12345");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page).toHaveURL(/\/app\/dashboard$/);

    await page.goto("/app/security");
    await page.getByRole("button", { name: "Enable TOTP" }).click();
    await expect(page).toHaveURL(/\/app\/security$/);
    await expect(page.getByText("TOTP enabled", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Disable TOTP" }).click();
    await expect(page).toHaveURL(/\/login\?security=totp-disabled$/);
    await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill("pass12345");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page).toHaveURL(/\/app\/dashboard$/);

    await page.goto("/app/help");
    await expect(page.getByRole("heading", { name: "Help Center" })).toBeVisible();
    await page.getByRole("link", { name: "Create Grid Strategy" }).click();
    await expect(page).toHaveURL(/\/help\/create-grid-strategy$/);
    await expect(page.getByRole("heading", { name: "Create Grid Strategy" })).toBeVisible();
    await expect(
      page.getByText(
        "Pre-flight validates exchange filters, available balance, and hedge mode before a strategy can start.",
        { exact: false },
      ),
    ).toBeVisible();
  });

  test("user commercial help center renders repository docs", async ({ page }) => {
    await page.goto("/help/telegram-notifications");

    await expect(page.getByRole("heading", { name: "Telegram Notifications" })).toBeVisible();
    await expect(
      page.getByText(
        "Bind a short-lived code in the web app, then send it to the Telegram bot to complete linking.",
        { exact: false },
      ),
    ).toBeVisible();
  });
});
