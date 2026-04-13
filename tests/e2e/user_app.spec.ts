import { expect, test } from "@playwright/test";
import { expectSessionCookie, loginViaPage, registerViaPage, uniqueEmail } from "./support/auth";

test("anonymous user is redirected away from app pages", async ({ page }) => {
  await page.goto("/app/dashboard");

  await expect(page).toHaveURL(/\/(?:en|zh)\/login\?next=%2F(?:en|zh)%2Fapp%2Fdashboard$/);
  await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
});

test("user can register and login through the browser before reviewing app areas", async ({
  page,
  context,
}) => {
  const email = uniqueEmail("trader");
  const password = "pass1234";

  await page.goto("/register");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();

  await expect(page).toHaveURL(/\/login\?/);
  await expect(page.getByText("Account created", { exact: false })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue(email);

  await loginViaPage(page, email, password);
  await expect(page).toHaveURL(/\/app\/dashboard$/);
  await expectSessionCookie(page);
  await expect(page.getByRole("heading", { name: "Trading Cockpit" })).toBeVisible();
  await expect(page.getByText("Recent Fills", { exact: false })).toBeVisible();

  await page.goto("/app/security");
  await expect(page).toHaveURL(/\/app\/security$/);
  await expect(page.getByRole("heading", { name: "Security Center" })).toBeVisible();

  await page.goto("/app/billing");
  await expect(page).toHaveURL(/\/app\/billing$/);
  await expect(page.getByRole("heading", { name: "Membership Center" })).toBeVisible();
  await expect(page.getByText("must match exactly", { exact: false }).first()).toBeVisible();

  await page.goto("/app/strategies");
  await expect(page).toHaveURL(/\/app\/strategies$/);
  await expect(page.getByRole("heading", { name: "Strategy Inventory" })).toBeVisible();

  await page.goto("/app/orders");
  await expect(page).toHaveURL(/\/app\/orders$/);
  await expect(page.getByRole("heading", { name: "Orders & History" })).toBeVisible();

  await page.goto("/app/notifications");
  await expect(page).toHaveURL(/\/app\/notifications$/);
  await expect(page.getByRole("heading", { name: "Notifications" })).toBeVisible();

  await page.goto("/help/expiry-reminder");
  await expect(page).toHaveURL(/\/help\/expiry-reminder$/);
  await expect(page.getByRole("heading", { name: "Expiry Reminder" }).first()).toBeVisible();

  await context.clearCookies();
  await page.goto("/app/dashboard");
  await expect(page).toHaveURL(/\/(?:en|zh)\/login\?next=%2F(?:en|zh)%2Fapp%2Fdashboard$/);
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
  await expect(page).toHaveURL(/\/login\?/);
  await expect(page.getByText("Account created", { exact: false })).toBeVisible();

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

test("strategy workspace supports per-grid custom editing and draft deletion", async ({ page }) => {
  const email = uniqueEmail("strategy");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByLabel("Grid Count").fill("3");
  await page.getByLabel("Editor Mode").selectOption("custom");
  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  await page.getByRole("button", { name: "Apply Batch Defaults" }).click();
  await expect(page.locator('[data-level-editor="true"]')).toBeVisible();

  const editor = page.locator('[data-level-editor="true"]');
  await editor.getByLabel("Grid Price").first().fill("1700");
  await editor.getByLabel("Grid Price").nth(1).fill("1750");
  await editor.getByLabel("Grid Price").nth(2).fill("1800");
  await editor.getByLabel("Quote Amount (USDT)").first().fill("50");
  await editor.getByLabel("Quote Amount (USDT)").nth(1).fill("60");
  await editor.getByLabel("Quote Amount (USDT)").nth(2).fill("70");
  await editor.getByLabel("Grid Take Profit (%)").first().fill("1.2");
  await editor.getByLabel("Grid Take Profit (%)").nth(1).fill("1.4");
  await editor.getByLabel("Grid Take Profit (%)").nth(2).fill("1.6");
  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();

  await page.getByRole("button", { name: "Create Bot" }).click();
  await expect(page).toHaveURL(/\/app\/strategies\/[^/]+\?notice=draft-saved/);
  await expect(page.getByRole("button", { name: "Delete Strategy" })).toBeVisible();
  await expect(page.getByText("Realized PnL", { exact: false })).toHaveCount(0);

  await page.getByRole("button", { name: "Delete Strategy" }).click();
  await expect(page).toHaveURL(/\/app\/strategies\?notice=strategy-deleted/);
  await expect(page.getByText("ETH Trend Grid", { exact: true })).not.toBeVisible();
});

test("strategy workspace can apply batch defaults into per-grid custom editing", async ({ page }) => {
  const email = uniqueEmail("strategy-batch-defaults");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByLabel("Editor Mode").selectOption("custom");
  await expect(page.getByLabel("Grid Count")).not.toHaveAttribute("readonly", "");
  await expect(page.getByLabel("Covered Range (%)")).toBeVisible();
  await page.getByLabel("Grid Count").fill("4");
  await page.getByLabel("Covered Range (%)").fill("8");
  await page.getByLabel("Per-grid Quote Amount (USDT)").fill("88");
  await page.getByLabel("Grid Take Profit (%)").first().fill("1.3");
  await page.getByRole("button", { name: "Apply Batch Defaults" }).click();

  const editor = page.locator('[data-level-editor="true"]');
  await expect(editor.getByText("L4", { exact: true })).toBeVisible();
  await expect(editor.getByLabel("Spacing vs Prev (%)").nth(1)).not.toHaveValue("");
  await expect(editor.getByLabel("Spacing vs Prev (%)").nth(2)).not.toHaveValue("");
  await expect(editor.getByLabel("Spacing vs Prev (%)").nth(3)).not.toHaveValue("");
  await expect(editor.getByLabel("Quote Amount (USDT)").first()).toHaveValue("88");
  await expect(editor.getByLabel("Quote Amount (USDT)").nth(1)).toHaveValue("88");
  await expect(editor.getByLabel("Grid Take Profit (%)").first()).toHaveValue("1.3");
  await expect(editor.getByLabel("Grid Take Profit (%)").nth(3)).toHaveValue("1.3");
});

test("strategy workspace warns when overall take profit may preempt per-grid exits", async ({ page }) => {
  const email = uniqueEmail("strategy-warning");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  await page.getByLabel("Anchor Price").fill("100");
  await page.getByLabel("Grid Count").fill("4");
  await page.getByLabel("Covered Range (%)").fill("4");
  await page.getByLabel("Grid Take Profit (%)").first().fill("2");
  await page.getByLabel("Overall Take Profit (%)").fill("1");

  await expect(
    page.getByRole("heading", { name: "Overall take profit may trigger before the grid take-profit plan" }),
  ).toBeVisible();
});

test("new strategy requires explicit symbol selection and batch actions stay opt-in", async ({ page }) => {
  const email = uniqueEmail("strategy-selection");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page.locator('input[name="symbol"]')).toHaveValue("");

  await page.getByRole("button", { name: "Create Bot" }).click();
  await expect(page).toHaveURL(/\/app\/strategies\/new\?error=/);
  await expect(page.getByText("Symbol must be selected from the search results.", { exact: false })).toBeVisible();

  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  await expect(page.locator('input[name="symbol"]')).toHaveValue("ETHUSDT");

  await page.getByRole("button", { name: "Create Bot" }).click();
  await expect(page).toHaveURL(/\/app\/strategies\/[^/]+\?notice=draft-saved/);

  await page.goto("/app/strategies");
  await expect(page.getByRole("button", { name: "Batch Start" })).toBeDisabled();
  const selector = page.locator('tbody input[type="checkbox"]').first();
  await selector.check();
  await expect(page.getByRole("button", { name: "Batch Start" })).toBeEnabled();
});


test("strategy workspace splits ordinary and classic definition sections", async ({ page }) => {
  const email = uniqueEmail("strategy-type-split");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  await expect(page.getByLabel("Strategy Type")).toHaveValue("ordinary_grid");
  await expect(page.getByLabel("Covered Range (%)")).toBeVisible();
  await expect(page.getByLabel("Upper Range (%)")).toHaveCount(0);
  await expect(page.getByLabel("Lower Range (%)")).toHaveCount(0);

  await page.getByLabel("Strategy Type").selectOption("classic_bilateral_grid");
  await expect(page.getByLabel("Covered Range (%)")).toHaveCount(0);
  await expect(page.getByLabel("Upper Range (%)")).toBeVisible();
  await expect(page.getByLabel("Lower Range (%)")).toBeVisible();
});

test("strategy preview updates immediately for ordinary and classic layouts without TP guides", async ({ page }) => {
  const email = uniqueEmail("strategy-preview-contract");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  const preview = page.locator('[data-strategy-preview="true"]');
  await expect(preview.locator('[data-preview-layout="ordinary"]')).toBeVisible();
  await expect(preview.locator('[data-preview-anchor="true"]')).toContainText("Anchor Price");
  await expect(preview.locator('[data-preview-range="ordinary"]')).toContainText("6");
  await expect(preview.getByText("TP", { exact: false })).toHaveCount(0);

  await page.getByLabel("Covered Range (%)").fill("9");
  await expect(preview.locator('[data-preview-range="ordinary"]')).toContainText("9");

  await page.getByLabel("Strategy Type").selectOption("classic_bilateral_grid");
  await page.getByLabel("Upper Range (%)").fill("4");
  await page.getByLabel("Lower Range (%)").fill("3");
  await expect(preview.locator('[data-preview-layout="classic"]')).toBeVisible();
  await expect(preview.locator('[data-preview-center="true"]')).toContainText("Center Price");
  await expect(preview.locator('[data-preview-range="classic-upper"]')).toContainText("4");
  await expect(preview.locator('[data-preview-range="classic-lower"]')).toContainText("3");
  await expect(preview.getByText("TP", { exact: false })).toHaveCount(0);
});


test("classic bilateral batch builder keeps a single level when grid count is 1", async ({ page }) => {
  const email = uniqueEmail("strategy-classic-one-level");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await expect(page).toHaveURL(/\/app\/strategies\/new$/);

  await page.getByRole("button", { name: /ETHUSDT/ }).first().click();
  await page.getByLabel("Strategy Type").selectOption("classic_bilateral_grid");
  await page.getByLabel("Grid Count").fill("1");
  await page.getByLabel("Upper Range (%)").fill("4");
  await page.getByLabel("Lower Range (%)").fill("3");

  const editor = page.locator('[data-level-editor="true"]');
  await expect(editor.getByText("L1", { exact: true })).toBeVisible();
  await expect(editor.getByText("L2", { exact: true })).toHaveCount(0);
});
