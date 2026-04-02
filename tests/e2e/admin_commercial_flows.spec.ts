import { expect, test, type APIRequestContext } from "@playwright/test";

const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const ADMIN_EMAIL = "admin@example.com";
const ADMIN_PASSWORD = "pass1234";

test.describe("admin commercial", () => {
  test("operator workflows cover memberships, deposits, pools, templates, strategies, sweeps, audit, and system config", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = await createAdminSession(request);
    await context.addCookies([
      {
        domain: "localhost",
        httpOnly: true,
        name: "session_token",
        path: "/",
        sameSite: "Lax",
        value: adminSessionToken,
      },
    ]);

    await page.goto("/admin/dashboard");
    await expect(page.getByRole("heading", { name: "Admin Dashboard" })).toBeVisible();
    await expect(page.getByText("Memberships needing action", { exact: true })).toBeVisible();
    await expect(page.getByText("Deposit exception queue", { exact: true })).toBeVisible();
    await expect(page.getByText("Runtime incidents", { exact: true })).toBeVisible();

    await page.getByRole("link", { name: "Memberships" }).click();
    await expect(page).toHaveURL(/\/admin\/memberships$/);
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await page.getByLabel("Membership state").selectOption("grace");
    await page.getByRole("button", { name: "Apply filters" }).click();
    await expect(page).toHaveURL(/state=grace/);
    await expect(page.getByText("miles@example.com", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Extend miles@example.com by 30 days" }).click();
    await expect(page.getByText("Membership updated", { exact: true })).toBeVisible();
    await expect(page.getByText("Extended to 2026-05-17", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Deposits" }).click();
    await expect(page).toHaveURL(/\/admin\/deposits$/);
    await expect(page.getByRole("heading", { name: "Abnormal Deposit Handling" })).toBeVisible();
    await expect(page.getByText("ORD-4201", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Resolve ORD-4201 as refunded" }).click();
    await expect(page.getByText("Deposit case updated", { exact: true })).toBeVisible();
    await expect(page.getByText("Refunded after user contact", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Address pools" }).click();
    await expect(page).toHaveURL(/\/admin\/address-pools$/);
    await expect(page.getByRole("heading", { name: "Address Pool Expansion" })).toBeVisible();
    await page.getByLabel("Chain").selectOption("bsc");
    await page.getByLabel("Expand by").fill("3");
    await page.getByRole("button", { name: "Expand BSC pool" }).click();
    await expect(page.getByText("Pool capacity updated", { exact: true })).toBeVisible();
    await expect(page.getByText("BSC now has 10 total addresses", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Templates" }).click();
    await expect(page).toHaveURL(/\/admin\/templates$/);
    await expect(page.getByRole("heading", { name: "Template Management" })).toBeVisible();
    await page.getByLabel("Template name").fill("ADA Trend Rider");
    await page.getByLabel("Market").selectOption("spot");
    await page.getByLabel("Strategy mode").selectOption("buy-only");
    await page.getByRole("button", { name: "Create template" }).click();
    await expect(page.getByText("Template saved", { exact: true })).toBeVisible();
    await expect(page.getByText("ADA Trend Rider", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Publish ADA Trend Rider" }).click();
    await expect(page.getByText("Template published", { exact: true })).toBeVisible();

    await page.getByRole("link", { name: "Strategies" }).click();
    await expect(page).toHaveURL(/\/admin\/strategies$/);
    await expect(page.getByRole("heading", { name: "Strategy Oversight" })).toBeVisible();
    await page.getByLabel("Runtime state").selectOption("error_paused");
    await page.getByRole("button", { name: "Apply filters" }).click();
    await expect(page).toHaveURL(/state=error_paused/);
    await expect(page.getByText("ETH Short Ladder", { exact: true })).toBeVisible();
    await expect(page.getByText("Hedge mode must be enabled before futures strategy restart.", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Sweeps" }).click();
    await expect(page).toHaveURL(/\/admin\/sweeps$/);
    await expect(page.getByRole("heading", { name: "Sweep Job Visibility" })).toBeVisible();
    await expect(page.getByText("Queued treasury jobs", { exact: true })).toBeVisible();
    await expect(page.getByText("bsc_pool_02", { exact: true })).toBeVisible();

    await page.getByRole("link", { name: "Audit" }).click();
    await expect(page).toHaveURL(/\/admin\/audit$/);
    await expect(page.getByRole("heading", { name: "Audit Log Review" })).toBeVisible();
    await page.getByLabel("Action domain").selectOption("membership");
    await page.getByRole("button", { name: "Apply filters" }).click();
    await expect(page).toHaveURL(/domain=membership/);
    await expect(page.getByText("membership.extend", { exact: true })).toBeVisible();
    await expect(page.getByText("pool.expand", { exact: true })).not.toBeVisible();

    await page.getByRole("link", { name: "System" }).click();
    await expect(page).toHaveURL(/\/admin\/system$/);
    await expect(page.getByRole("heading", { name: "System Configuration" })).toBeVisible();
    await page.getByLabel("BSC confirmations").fill("18");
    await page.getByRole("button", { name: "Save billing configuration" }).click();
    await expect(page.getByText("System configuration saved", { exact: true })).toBeVisible();
    await expect(page.getByText("bsc_confirmations", { exact: true })).toBeVisible();
    await expect(page.getByText("18", { exact: true })).toBeVisible();

    await page.goto("/admin/dashboard");
    await expect(page.getByText("3 open abnormal cases", { exact: false })).toBeVisible();
    await expect(page.getByText("12 active templates", { exact: false })).toBeVisible();
    await expect(page.getByText("Latest audit: system.update", { exact: false })).toBeVisible();
  });
});

async function createAdminSession(request: APIRequestContext) {
  await ensureAdminUser(request);
  const preTotpSessionToken = await login(request);
  const enabled = await request.post(`${AUTH_API_BASE_URL}/security/totp/enable`, {
    data: { email: ADMIN_EMAIL },
    headers: {
      authorization: `Bearer ${preTotpSessionToken}`,
    },
  });

  expect(enabled.ok()).toBeTruthy();
  const payload = (await enabled.json()) as { code?: string };
  expect(typeof payload.code).toBe("string");

  return login(request, payload.code);
}

async function ensureAdminUser(request: APIRequestContext) {
  const register = await request.post(`${AUTH_API_BASE_URL}/auth/register`, {
    data: {
      email: ADMIN_EMAIL,
      password: ADMIN_PASSWORD,
    },
  });

  if (register.status() === 409) {
    return;
  }

  expect(register.ok()).toBeTruthy();
  const payload = (await register.json()) as { verification_code?: string };
  expect(typeof payload.verification_code).toBe("string");

  const verify = await request.post(`${AUTH_API_BASE_URL}/auth/verify-email`, {
    data: {
      code: payload.verification_code,
      email: ADMIN_EMAIL,
    },
  });

  expect(verify.ok()).toBeTruthy();
}

async function login(request: APIRequestContext, totpCode?: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/auth/login`, {
    data: {
      email: ADMIN_EMAIL,
      password: ADMIN_PASSWORD,
      totp_code: totpCode ?? null,
    },
  });

  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { session_token?: string };
  expect(typeof payload.session_token).toBe("string");
  return payload.session_token!;
}
