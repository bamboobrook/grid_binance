import { expect, test } from "@playwright/test";

test("user can review billing, security, and strategies", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("main")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Grid Binance" })).toBeVisible();

  await page.getByRole("link", { name: "Registration Entry" }).click();
  await expect(page).toHaveURL(/\/register$/);
  await expect(page.getByRole("heading", { name: "Register" })).toBeVisible();

  await page.goto("/app/dashboard");
  await expect(page.getByRole("heading", { name: "User Dashboard" })).toBeVisible();
  await expect(page.getByText("Expiry reminder flow")).toBeVisible();

  await page.getByRole("link", { name: "Security Center" }).click();
  await expect(page).toHaveURL(/\/app\/security$/);
  await expect(page.getByRole("heading", { name: "Security Center" })).toBeVisible();

  await page.goto("/app/dashboard");

  await page.getByRole("link", { name: "Billing Center" }).click();
  await expect(page).toHaveURL(/\/app\/billing$/);
  await expect(page.getByRole("heading", { name: "Billing Center" })).toBeVisible();
  await expect(page.getByText("Next renewal")).toBeVisible();

  await page.getByRole("link", { name: "Strategy Workspace" }).click();
  await expect(page).toHaveURL(/\/app\/strategies\/grid-btc$/);
  await expect(page.getByRole("heading", { name: "Strategy Workspace" })).toBeVisible();

  await page.getByRole("link", { name: "Analytics" }).click();
  await expect(page).toHaveURL(/\/app\/analytics$/);
  await expect(page.getByRole("heading", { name: "Analytics" })).toBeVisible();

  await page.goto("/app/strategies/grid-btc");
  await page.getByRole("link", { name: "Help Center" }).click();
  await expect(page).toHaveURL(/\/help\/expiry-reminder$/);
  await expect(page.getByRole("heading", { name: "Expiry Reminder" })).toBeVisible();
});
