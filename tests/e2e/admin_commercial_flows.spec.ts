import { expect, test, type APIRequestContext } from "@playwright/test";

const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const ADMIN_EMAIL = "admin@example.com";
const ADMIN_PASSWORD = "pass1234";
const SUPER_ADMIN_EMAIL = "super-admin@example.com";

let uniqueCounter = 0;

test.describe("admin commercial", () => {
  test("operator admin sees restricted commercial controls but can process abnormal orders and runtime review", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = await createAdminSession(request, ADMIN_EMAIL);
    const creditEmail = uniqueEmail("credit");
    const rejectEmail = uniqueEmail("reject");
    const strategyName = `admin-draft-${Date.now()}`;

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
    await expect(page.getByText("operator_admin", { exact: false })).toBeVisible();
    await expect(page.getByText("Restricted permission boundary", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Memberships" }).click();
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await expect(page.getByText("Plan & pricing management", { exact: false })).toBeVisible();
    await expect(page.getByText("super_admin required", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open membership" })).not.toBeVisible();

    await page.getByRole("link", { name: "Deposits" }).click();
    await expect(page.getByRole("heading", { name: "Abnormal Deposit Handling" })).toBeVisible();
    await expect(page.getByText(seeded.rejectTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText(seeded.creditTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText("Manual credit target order", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: `Reject ${seeded.rejectTxHash}` }).click();
    await expect(page.getByText("Deposit result: manual_rejected", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: `Credit ${seeded.creditTxHash} to membership` }).click();
    await expect(page.getByText("Deposit result: manual_approved", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Address pools" }).click();
    await expect(page.getByRole("heading", { name: "Address Pool Inventory" })).toBeVisible();
    await expect(page.getByText("bsc-addr-1", { exact: true })).toBeVisible();
    await expect(page.getByText("Enabled inventory", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Add or enable address" })).not.toBeVisible();

    await page.getByRole("link", { name: "Templates" }).click();
    await expect(page.getByRole("heading", { name: "Template Management" })).toBeVisible();
    await expect(page.getByText("Template changes require super_admin", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Create template" })).not.toBeVisible();

    await page.getByRole("link", { name: "Strategies" }).click();
    await expect(page.getByRole("heading", { name: "Strategy Oversight" })).toBeVisible();
    await expect(page.getByText("Runtime overview", { exact: false })).toBeVisible();
    await expect(page.getByText(strategyName, { exact: true })).toBeVisible();
    await expect(page.getByText("Active orders", { exact: false })).toBeVisible();
    await expect(page.getByText("Last pre-flight", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Audit" }).click();
    await expect(page.getByRole("heading", { name: "Audit Review" })).toBeVisible();
    await expect(page.getByText("Before / after summary", { exact: false })).toBeVisible();
    await expect(page.getByRole("cell", { name: "decision reject" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "decision credit_membership" }).first()).toBeVisible();
  });

  test("super admin manages pricing, memberships, templates, inventory, sweeps, and audit-backed system controls", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = await createAdminSession(request, SUPER_ADMIN_EMAIL);
    const memberEmail = uniqueEmail("member");
    const creditEmail = uniqueEmail("credit");
    const rejectEmail = uniqueEmail("reject");
    const strategyName = `admin-draft-${Date.now()}`;
    const extraAddress = `bsc-ops-${Date.now()}`;

    await seedAdminCommercialData(request, adminSessionToken, {
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
    await expect(page.getByText("super_admin", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Memberships" }).click();
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Plan & pricing management" })).toBeVisible();
    await page.getByLabel("Plan code").fill("monthly");
    await page.getByLabel("Display name").fill("Monthly");
    await page.getByLabel("Plan duration days").fill("31");
    await page.getByLabel("BSC / USDT price").fill("24.50");
    await page.getByLabel("ETH / USDT price").fill("24.80");
    await page.getByLabel("SOL / USDC price").fill("24.10");
    await page.getByRole("button", { name: "Save plan pricing" }).click();
    await expect(page.getByText("Plan pricing saved", { exact: false })).toBeVisible();
    await expect(page.getByText("24.50", { exact: false })).toBeVisible();
    await page.getByLabel("Member email").fill(memberEmail);
    await page.getByLabel("Membership duration days").fill("30");
    await page.getByRole("button", { name: "Open membership" }).click();
    await expect(page.getByText(`Target: ${memberEmail}`, { exact: false })).toBeVisible();
    await expect(page.getByText("Status: Active", { exact: false })).toBeVisible();
    await page.getByLabel("Membership duration days").fill("15");
    await page.getByRole("button", { name: "Extend membership" }).click();
    await expect(page.getByText("Last action: extend", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Freeze membership" }).click();
    await expect(page.getByText("Status: Frozen", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Unfreeze membership" }).click();
    await expect(page.getByText("Last action: unfreeze", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Revoke membership" }).click();
    await expect(page.getByText("Status: Revoked", { exact: false })).toBeVisible();

    await page.getByRole("link", { name: "Address pools" }).click();
    await expect(page.getByRole("heading", { name: "Address Pool Inventory" })).toBeVisible();
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

    await page.getByRole("link", { name: "Sweeps" }).click();
    await expect(page.getByRole("heading", { name: "Sweep Operations" })).toBeVisible();
    await page.getByLabel("Treasury address").fill("treasury-bsc-main");
    await page.getByLabel("Source address").fill("bsc-addr-2");
    await page.getByLabel("Sweep amount").fill("18.50000000");
    await page.getByRole("button", { name: "Request sweep" }).click();
    await expect(page.getByText("Sweep request submitted", { exact: false })).toBeVisible();
    await expect(page.getByRole("cell", { name: "treasury-bsc-main" }).first()).toBeVisible();

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

    await page.getByRole("link", { name: "Audit" }).click();
    await expect(page.getByRole("heading", { name: "Audit Review" })).toBeVisible();
    await expect(page.getByRole("cell", { name: "strategy.template_created" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "membership.plan_config_updated" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "treasury.sweep_requested" }).first()).toBeVisible();
    await expect(page.getByText("session role super_admin", { exact: false })).toBeVisible();
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

  await createStrategy(request, creditUserToken, input.strategyName);
  await createSweepJob(request, adminSessionToken);

  return {
    creditOrderId: creditOrder.orderId,
    creditTxHash,
    rejectTxHash,
  };
}

async function createAdminSession(request: APIRequestContext, email: string) {
  await ensureVerifiedUser(request, email, ADMIN_PASSWORD);
  const preTotpSessionToken = await login(request, email, ADMIN_PASSWORD);
  const enabled = await request.post(`${AUTH_API_BASE_URL}/security/totp/enable`, {
    data: { email },
    headers: {
      authorization: `Bearer ${preTotpSessionToken}`,
    },
  });

  expect(enabled.ok()).toBeTruthy();
  const payload = (await enabled.json()) as { code?: string };
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
  if (!response.ok()) {
    throw new Error(`matchOrderAsAbnormal failed ${response.status()} ${await response.text()}`);
  }
}

async function createSweepJob(request: APIRequestContext, adminSessionToken: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/admin/sweeps`, {
    data: {
      chain: "BSC",
      asset: "USDT",
      treasury_address: "treasury-bsc-main",
      requested_at: new Date().toISOString(),
      transfers: [{ from_address: "bsc-addr-1", amount: "20.00000000" }],
    },
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok()).toBeTruthy();
}

async function createStrategy(request: APIRequestContext, sessionToken: string, name: string) {
  const response = await request.post(`${AUTH_API_BASE_URL}/strategies`, {
    data: {
      name,
      symbol: "ADAUSDT",
      market: "Spot",
      mode: "SpotClassic",
      generation: "Custom",
      levels: [
        { entry_price: "1.00", quantity: "10", take_profit_bps: 120, trailing_bps: null },
        { entry_price: "1.10", quantity: "10", take_profit_bps: 140, trailing_bps: null },
      ],
      membership_ready: true,
      exchange_ready: true,
      permissions_ready: true,
      withdrawals_disabled: true,
      hedge_mode_ready: true,
      symbol_ready: true,
      filters_ready: true,
      margin_ready: true,
      conflict_ready: true,
      balance_ready: true,
      overall_take_profit_bps: null,
      overall_stop_loss_bps: null,
      post_trigger_action: "Stop",
    },
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
  });
  if (!response.ok()) {
    throw new Error(`createStrategy failed ${response.status()} ${await response.text()}`);
  }
}

function uniqueEmail(prefix: string) {
  uniqueCounter += 1;
  return `${prefix}-${Date.now()}-${uniqueCounter}@example.com`;
}
