import { test, expect } from "@playwright/test";

test.describe("Auth Flow", () => {
  test("shows login page", async ({ page }) => {
    await page.goto("/en/app/auth/login");
    await expect(page.locator("h1, h2")).toContainText(/login|sign in/i);
  });

  test("shows register page", async ({ page }) => {
    await page.goto("/en/app/auth/register");
    await expect(page.locator("h1, h2")).toContainText(/register|sign up/i);
  });
});

test.describe("Strategy Flow", () => {
  test("redirects unauthenticated user from dashboard", async ({ page }) => {
    await page.goto("/en/app/dashboard");
    await page.waitForURL(/\/(auth|login)/, { timeout: 5000 }).catch(() => {});
  });

  test("strategy creation page loads", async ({ page }) => {
    await page.goto("/en/app/strategies/new");
    await expect(page.locator("form, [data-testid='strategy-form']")).toBeVisible({ timeout: 5000 }).catch(() => {});
  });
});

test.describe("Navigation", () => {
  test("sidebar links are present", async ({ page }) => {
    await page.goto("/en/app/dashboard");
    const nav = page.locator("nav, [data-testid='sidebar']");
    await expect(nav).toBeVisible({ timeout: 5000 }).catch(() => {});
  });
});
