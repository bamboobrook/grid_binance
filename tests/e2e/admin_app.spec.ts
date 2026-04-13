import { expect, test, type APIRequestContext, type BrowserContext } from "@playwright/test";

const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const ADMIN_PASSWORD = "pass1234";
const SUPER_ADMIN_EMAIL = "admin-app-super@example.com";

let adminSessionToken = "";

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ request }) => {
  adminSessionToken = await createAdminSession(request, SUPER_ADMIN_EMAIL);
});

test("anonymous user is redirected away from admin pages", async ({ page }) => {
  await page.goto("/admin/dashboard");

  await expect(page).toHaveURL(/\/(?:en|zh)\/login\?next=%2F(?:en|zh)%2Fadmin%2Fdashboard$/);
  await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
});

test("admin can browse the current admin page map with a TOTP-backed session", async ({
  context,
  page,
}) => {
  await addAdminSessionCookie(context);

  const pages = [
    { heading: "Operations Overview", path: "/admin/dashboard" },
    { heading: "User Management", path: "/admin/users" },
    { heading: "Membership Operations", path: "/admin/memberships" },
    { heading: "Abnormal Deposit Handling", path: "/admin/deposits" },
    { heading: "Address Pool Inventory", path: "/admin/address-pools" },
    { heading: "Template Management", path: "/admin/templates" },
    { heading: "Strategy Oversight", path: "/admin/strategies" },
    { heading: "Sweep Operations", path: "/admin/sweeps" },
    { heading: "System Settings", path: "/admin/system" },
    { heading: "Audit Review", path: "/admin/audit" },
  ] as const;

  for (const entry of pages) {
    await page.goto(entry.path);
    await expect(page).toHaveURL(new RegExp(`${entry.path.replace(/\//g, "\\/")}$`));
    await expect(page.getByRole("heading", { name: entry.heading })).toBeVisible();
  }
});

test("admin can use the legacy billing route and land on deposits", async ({
  context,
  page,
}) => {
  await addAdminSessionCookie(context);

  await page.goto("/admin/billing");

  await expect(page).toHaveURL(/\/admin\/deposits$/);
  await expect(page.getByRole("heading", { name: "Abnormal Deposit Handling" })).toBeVisible();
});

async function addAdminSessionCookie(context: BrowserContext) {
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
}

async function createAdminSession(request: APIRequestContext, email: string) {
  await ensureVerifiedUser(request, email, ADMIN_PASSWORD);
  const bootstrap = await request.post(`${AUTH_API_BASE_URL}/auth/admin-bootstrap`, {
    data: { email, password: ADMIN_PASSWORD },
  });

  expect(bootstrap.ok()).toBeTruthy();
  const payload = (await bootstrap.json()) as { code?: string };
  expect(typeof payload.code).toBe("string");

  const sessionToken = await login(request, email, ADMIN_PASSWORD, payload.code);
  const profile = await request.get(`${AUTH_API_BASE_URL}/profile`, {
    headers: { authorization: `Bearer ${sessionToken}` },
  });

  if (!profile.ok()) {
    throw new Error(`profile check failed ${profile.status()} ${await profile.text()}`);
  }

  const profileBody = await profile.text();
  if (!profileBody.includes('"admin_access_granted":true')) {
    throw new Error(`admin session missing access ${email} ${profileBody}`);
  }

  return sessionToken;
}

async function ensureVerifiedUser(request: APIRequestContext, email: string, password: string) {
  const register = await request.post(`${AUTH_API_BASE_URL}/auth/register`, {
    data: { email, password },
  });

  if (register.status() === 409) {
    return;
  }

  expect(register.ok()).toBeTruthy();
  const payload = (await register.json()) as { user_id?: number };
  expect(typeof payload.user_id).toBe("number");
}

async function login(
  request: APIRequestContext,
  email: string,
  password: string,
  totpCode?: string,
) {
  const response = await request.post(`${AUTH_API_BASE_URL}/auth/login`, {
    data: { email, password, totp_code: totpCode ?? null },
  });

  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { session_token?: string };
  expect(typeof payload.session_token).toBe("string");
  return payload.session_token!;
}
