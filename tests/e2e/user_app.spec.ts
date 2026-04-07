import { expect, test } from "@playwright/test";
import { expectSessionCookie, loginViaPage, registerViaPage, uniqueEmail } from "./support/auth";

test("anonymous user is redirected away from app pages", async ({ page }) => {
  await page.goto("/app/dashboard");

  await expect(page).toHaveURL(/\/login\?next=%2Fapp%2Fdashboard$/);
  await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
});

test("user can register verify email and login through the browser before reviewing app areas", async ({
  page,
  context,
}) => {
  const email = uniqueEmail("trader");
  const password = "pass1234";

  await page.goto("/register");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();

  await expect(page).toHaveURL(/\/verify-email\?/);
  await expect(page.getByRole("heading", { name: "Verify Email" })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue(email);

  const cookies = await page.context().cookies();
  const verificationCode = cookies.find((cookie) => cookie.name === "pending_verify_code")?.value ?? "";
  expect(verificationCode).toMatch(/^\d{6}$/);
  await page.getByLabel("Verification code").fill(verificationCode);

  await page.getByRole("button", { name: "Verify email" }).click();
  await expect(page).toHaveURL(/\/login\?/);
  await expect(page.getByText("Email verified", { exact: false })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue(email);

  await loginViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
  await expect(page.getByRole("heading", { name: "User Dashboard" })).toBeVisible();
  await expect(page.getByText("Expiry reminder flow", { exact: true })).toBeVisible();

  await page.goto("/app/security");
  await expect(page).toHaveURL(/\/app\/security$/);
  await expect(page.getByRole("heading", { name: "Security Center" })).toBeVisible();

  await page.goto("/app/billing");
  await expect(page).toHaveURL(/\/app\/billing$/);
  await expect(page.getByRole("heading", { name: "Billing Center" })).toBeVisible();
  await expect(page.getByText("Exact chain, token, and amount are all required for automatic confirmation.", { exact: false })).toBeVisible();

  await page.goto("/app/strategies");
  await expect(page).toHaveURL(/\/app\/strategies$/);
  await expect(page.getByRole("heading", { name: "Strategies" })).toBeVisible();

  await page.goto("/app/orders");
  await expect(page).toHaveURL(/\/app\/orders$/);
  await expect(page.getByRole("heading", { name: "Orders & History" })).toBeVisible();

  await page.goto("/help/expiry-reminder");
  await expect(page).toHaveURL(/\/help\/expiry-reminder$/);
  await expect(page.getByRole("heading", { name: "Expiry Reminder" })).toBeVisible();

  await context.clearCookies();
  await page.goto("/app/dashboard");
  await expect(page).toHaveURL(/\/login\?next=%2Fapp%2Fdashboard$/);
  await loginViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
});

test("user can complete a TOTP login challenge through the browser", async ({
  page,
  context,
}) => {
  const email = uniqueEmail("totp");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);

  await page.goto("/app/security");
  await page.getByRole("button", { name: "Enable TOTP" }).click();
  await expect(page).toHaveURL(/\/app\/security\?/);
  await expect(page.getByText("TOTP enabled", { exact: true })).toBeVisible();

  const totpSecret = await page.getByLabel("TOTP secret").inputValue();
  const totpCode = await page.getByLabel("Current TOTP code").inputValue();
  expect(totpSecret).not.toEqual("");
  expect(totpCode).toMatch(/^\d{6}$/);
  await expect(page.getByText("TOTP: Enabled", { exact: false })).toBeVisible();

  await context.clearCookies();
  await page.goto("/login");
  await loginViaPage(page, email, password);
  await expect(page).toHaveURL(/\/login\?.*totp=1/);
  await expect(page.getByLabel("Email")).toHaveValue(email);
  await expect(page.getByLabel("TOTP code")).toBeVisible();
  await expect(page.getByText("totp code required", { exact: false })).toBeVisible();

  await loginViaPage(page, email, password, { totpCode });
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
});

test("user can request and confirm a password reset through the browser", async ({ page }) => {
  const email = uniqueEmail("reset");
  const password = "pass1234";
  const nextPassword = "pass5678";

  await page.goto("/register");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page).toHaveURL(/\/verify-email\?/);
  const registerCookies = await page.context().cookies();
  const initialVerificationCode = registerCookies.find((cookie) => cookie.name === "pending_verify_code")?.value ?? "";
  await page.getByLabel("Verification code").fill(initialVerificationCode);
  await page.getByRole("button", { name: "Verify email" }).click();
  await expect(page).toHaveURL(/\/login\?/);

  await page.goto("/password-reset");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Send reset code" }).click();
  await expect(page).toHaveURL(/\/password-reset\?.*step=confirm/);
  await expect(page.getByText("Reset code issued", { exact: true })).toBeVisible();

  const resetCookies = await page.context().cookies();
  const resetCode = resetCookies.find((cookie) => cookie.name === "pending_reset_code")?.value ?? "";
  expect(resetCode).toMatch(/^\d{6}$/);
  await page.getByLabel("Reset code").fill(resetCode);
  await page.getByLabel("New password").fill(nextPassword);
  await page.getByRole("button", { name: "Reset password" }).click();
  await expect(page).toHaveURL(/\/login\?/);
  await expect(page.getByText("Password reset complete", { exact: false })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue(email);

  await loginViaPage(page, email, password);
  await expect(page.getByText("invalid credentials", { exact: false })).toBeVisible();
  await loginViaPage(page, email, nextPassword);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
});

test("invalid help slug returns 404", async ({ page }) => {
  const response = await page.goto("/help/not-a-real-article");

  expect(response?.status()).toBe(404);
  await expect(page.getByText("404")).toBeVisible();
});
