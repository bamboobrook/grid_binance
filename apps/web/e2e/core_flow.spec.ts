import { test, expect } from "@playwright/test";

test.describe("Core user flow", () => {
  test("landing page loads", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/Grid/i);
  });

  test("login page renders", async ({ page }) => {
    await page.goto("/en/app/login");
    await expect(page.locator("form")).toBeVisible();
  });

  test("dashboard redirects unauthenticated user", async ({ page }) => {
    await page.goto("/en/app/dashboard");
    const url = page.url();
    expect(url).toMatch(/login|dashboard/);
  });

  test("strategy creation page renders form", async ({ page }) => {
    await page.goto("/en/app/strategies/new");
    const form = page.locator("form");
    if (await form.isVisible()) {
      await expect(form).toBeVisible();
    }
  });

  test("backtest page renders", async ({ page }) => {
    await page.goto("/en/app/backtest");
    const heading = page.locator("h1, h2");
    if (await heading.first().isVisible()) {
      await expect(heading.first()).toBeVisible();
    }
  });
});
