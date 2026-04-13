import { expect, type Page } from "@playwright/test";

export function uniqueEmail(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@example.com`;
}

export async function registerViaPage(
  page: Page,
  email: string,
  password: string,
) {
  await page.goto("/register");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/login\?/);
  await expect(page.getByText("Account created", { exact: false })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue(email);
  await loginViaPage(page, email, password);
}

export async function loginViaPage(
  page: Page,
  email: string,
  password: string,
  options?: {
    totpCode?: string;
  },
) {
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  if (options?.totpCode) {
    await page.getByLabel("TOTP code").fill(options.totpCode);
  }
  await page.getByRole("button", { name: "Sign in" }).click();
}

export async function expectSessionCookie(page: Page) {
  await expect
    .poll(async () => {
      const cookies = await page.context().cookies();
      return cookies.some(
        (cookie) =>
          cookie.name === "session_token" && cookie.value.length > 0,
      );
    })
    .toBe(true);
}
