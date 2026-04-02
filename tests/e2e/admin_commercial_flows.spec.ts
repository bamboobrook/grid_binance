import { expect, test, type APIRequestContext } from "@playwright/test";

const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const ADMIN_EMAIL = "admin@example.com";
const ADMIN_PASSWORD = "pass1234";

let uniqueCounter = 0;

test.describe("admin commercial", () => {
  test("operator workflows use backend-backed admin data and required actions", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = await createAdminSession(request);
    const memberEmail = uniqueEmail("member");
    const creditEmail = uniqueEmail("credit");
    const rejectEmail = uniqueEmail("reject");
    const strategyName = `admin-draft-${Date.now()}`;
    const extraAddress = `bsc-ops-${Date.now()}`;

    const seeded = await seedAdminCommercialData(request, adminSessionToken, {
      creditEmail,
      rejectEmail,
      strategyName,
    });

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
    await expect(page.getByText("Admin access granted", { exact: false })).toBeVisible();
    await expect(page.getByText("super_admin", { exact: false })).not.toBeVisible();

    await page.getByRole("link", { name: "Memberships" }).click();
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await page.getByLabel("Member email").fill(memberEmail);
    await page.getByLabel("Duration days").fill("30");
    await page.getByRole("button", { name: "Open membership" }).click();
    await expect(page.getByText(`Target: ${memberEmail}`, { exact: false })).toBeVisible();
    await expect(page.getByText("Status: Active", { exact: false })).toBeVisible();
    await page.getByLabel("Duration days").fill("15");
    await page.getByRole("button", { name: "Extend membership" }).click();
    await expect(page.getByText("Last action: extend", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Freeze membership" }).click();
    await expect(page.getByText("Status: Frozen", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Unfreeze membership" }).click();
    await expect(page.getByText("Last action: unfreeze", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Revoke membership" }).click();
    await expect(page.getByText("Status: Revoked", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Deposits" }).click();
    await expect(page.getByRole("heading", { name: "Abnormal Deposit Handling" })).toBeVisible();
    await expect(page.getByText(seeded.rejectTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText(seeded.creditTxHash, { exact: true })).toBeVisible();
    await page.getByRole("button", { name: `Reject ${seeded.rejectTxHash}` }).click();
    await expect(page.getByText("Deposit result: manual_rejected", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: `Credit ${seeded.creditTxHash} to membership` }).click();
    await expect(page.getByText("Deposit result: manual_approved", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Address pools" }).click();
    await expect(page.getByRole("heading", { name: "Address Pool Inventory" })).toBeVisible();
    await expect(page.getByText("bsc-addr-1", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Disable bsc-addr-1" }).click();
    await expect(page.getByText("Updated address bsc-addr-1", { exact: false })).toBeVisible();
    await page.getByLabel("Chain").selectOption("BSC");
    await page.getByLabel("Address").fill(extraAddress);
    await page.getByRole("button", { name: "Add or enable address" }).click();
    await expect(page.getByText(extraAddress, { exact: true })).toBeVisible();

    await page.getByRole("link", { name: "Templates" }).click();
    await expect(page.getByRole("heading", { name: "Template Management" })).toBeVisible();
    await page.getByLabel("Template name").fill("ADA Trend Rider");
    await page.getByRole("button", { name: "Create template" }).click();
    await expect(page.getByText("Template created", { exact: false })).toBeVisible();
    await expect(page.getByText("ADA Trend Rider", { exact: true })).toBeVisible();

    await page.getByRole("link", { name: "Strategies" }).click();
    await expect(page.getByRole("heading", { name: "Strategy Oversight" })).toBeVisible();
    await page.getByLabel("Runtime state").selectOption("draft");
    await page.getByRole("button", { name: "Apply filters" }).click();
    await expect(page.getByText("No operator-visible strategies yet", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Sweeps" }).click();
    await expect(page.getByRole("heading", { name: "Sweep Job Visibility" })).toBeVisible();
    await expect(page.getByText("treasury-bsc-main", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Audit" }).click();
    await expect(page.getByRole("heading", { name: "Audit Review" })).toBeVisible();
    await expect(page.getByText("Backend audit records are written server-side", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "System" }).click();
    await expect(page.getByRole("heading", { name: "System Configuration" })).toBeVisible();
    await page.getByLabel("ETH confirmations").fill("18");
    await page.getByLabel("BSC confirmations").fill("15");
    await page.getByLabel("SOL confirmations").fill("22");
    await page.getByRole("button", { name: "Save confirmation policy" }).click();
    await expect(page.getByText("Confirmation policy saved", { exact: false })).toBeVisible();
    await expect(page.getByText("ETH 18", { exact: false })).toBeVisible();
    await expect(page.getByText("BSC 15", { exact: false })).toBeVisible();
    await expect(page.getByText("SOL 22", { exact: false })).toBeVisible();
  });
});

async function seedAdminCommercialData(
  request: APIRequestContext,
  adminSessionToken: string,
  input: { creditEmail: string; rejectEmail: string; strategyName: string },
) {
  const creditUserToken = await createVerifiedUserSession(request, input.creditEmail, "pass1234");
  const rejectUserToken = await createVerifiedUserSession(request, input.rejectEmail, "pass1234");

  const creditOrder = await createBillingOrder(request, creditUserToken, input.creditEmail);
  const rejectOrder = await createBillingOrder(request, rejectUserToken, input.rejectEmail);

  const creditTxHash = `tx-credit-${Date.now()}`;
  const rejectTxHash = `tx-reject-${Date.now()}`;

  await matchOrderAsAbnormal(request, adminSessionToken, {
    address: creditOrder.address,
    amount: creditOrder.amount,
    asset: "USDC",
    chain: "BSC",
    txHash: creditTxHash,
  });
  await matchOrderAsAbnormal(request, adminSessionToken, {
    address: rejectOrder.address,
    amount: "19.50000000",
    asset: "USDT",
    chain: "BSC",
    txHash: rejectTxHash,
  });

  await createSweepJob(request, adminSessionToken);

  return {
    creditOrderId: creditOrder.orderId,
    creditTxHash,
    rejectTxHash,
  };
}

async function createAdminSession(request: APIRequestContext) {
  await ensureVerifiedUser(request, ADMIN_EMAIL, ADMIN_PASSWORD);
  const preTotpSessionToken = await login(request, ADMIN_EMAIL, ADMIN_PASSWORD);
  const enabled = await request.post(`${AUTH_API_BASE_URL}/security/totp/enable`, {
    data: { email: ADMIN_EMAIL },
    headers: {
      authorization: `Bearer ${preTotpSessionToken}`,
    },
  });

  expect(enabled.ok()).toBeTruthy();
  const payload = (await enabled.json()) as { code?: string };
  expect(typeof payload.code).toBe("string");

  return login(request, ADMIN_EMAIL, ADMIN_PASSWORD, payload.code);
}

async function createVerifiedUserSession(request: APIRequestContext, email: string, password: string) {
  await ensureVerifiedUser(request, email, password);
  return login(request, email, password);
}

async function ensureVerifiedUser(request: APIRequestContext, email: string, password: string) {
  const register = await request.post(`${AUTH_API_BASE_URL}/auth/register`, {
    data: { email, password },
  });

  if (register.status() === 409) {
    return;
  }

  expect(register.ok()).toBeTruthy();
  const payload = (await register.json()) as { verification_code?: string };
  expect(typeof payload.verification_code).toBe("string");

  const verify = await request.post(`${AUTH_API_BASE_URL}/auth/verify-email`, {
    data: { code: payload.verification_code, email },
  });
  expect(verify.ok()).toBeTruthy();
}

async function login(request: APIRequestContext, email: string, password: string, totpCode?: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/auth/login`, {
    data: { email, password, totp_code: totpCode ?? null },
  });

  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { session_token?: string };
  expect(typeof payload.session_token).toBe("string");
  return payload.session_token!;
}

async function createBillingOrder(request: APIRequestContext, sessionToken: string, email: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/billing/orders`, {
    data: {
      asset: "USDT",
      at: undefined,
      chain: "BSC",
      email,
      plan_code: "monthly",
      requested_at: new Date().toISOString(),
    },
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { address?: string; amount?: string; order_id?: number };
  expect(typeof payload.address).toBe("string");
  expect(typeof payload.amount).toBe("string");
  expect(typeof payload.order_id).toBe("number");
  return { address: payload.address!, amount: payload.amount!, orderId: payload.order_id! };
}

async function matchOrderAsAbnormal(
  request: APIRequestContext,
  adminSessionToken: string,
  input: { address: string; amount: string; asset: string; chain: string; txHash: string },
) {
  const response = await request.post(`${AUTH_API_BASE_URL}/billing/orders/match`, {
    data: {
      address: input.address,
      amount: input.amount,
      asset: input.asset,
      chain: input.chain,
      observed_at: new Date().toISOString(),
      tx_hash: input.txHash,
    },
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok()).toBeTruthy();
}


async function createSweepJob(request: APIRequestContext, adminSessionToken: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/admin/sweeps`, {
    data: {
      chain: "BSC",
      asset: "USDT",
      treasury_address: "treasury-bsc-main",
      requested_at: new Date().toISOString(),
      transfers: [
        { from_address: "bsc-addr-1", amount: "20.00000000" },
      ],
    },
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok()).toBeTruthy();
}

function uniqueEmail(prefix: string) {
  uniqueCounter += 1;
  return `${prefix}-${Date.now()}-${uniqueCounter}@example.com`;
}
