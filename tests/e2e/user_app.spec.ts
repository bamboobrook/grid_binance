import { expect, test } from "@playwright/test";
import { expectSessionCookie, loginViaPage, registerViaPage, uniqueEmail } from "./support/auth";

test("anonymous user is redirected away from app pages", async ({ page }) => {
  await page.goto("/app/dashboard");

  await expect(page).toHaveURL(/\/login\?next=%2Fapp%2Fdashboard$/);
  await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
});

test("user can register and login through the browser before reviewing app areas", async ({
  page,
  context,
}) => {
  const email = uniqueEmail("trader");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
  await expect(page.getByRole("heading", { name: "User Dashboard" })).toBeVisible();
  await expect(page.getByText("Expiry reminder flow")).toBeVisible();

  await context.clearCookies();
  await page.goto("/app/dashboard");
  await expect(page).toHaveURL(/\/login\?next=%2Fapp%2Fdashboard$/);
  await loginViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);

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

test("invalid help slug returns 404", async ({ page }) => {
  const response = await page.goto("/help/not-a-real-article");

  expect(response?.status()).toBe(404);
  await expect(page.getByText("404")).toBeVisible();
});
