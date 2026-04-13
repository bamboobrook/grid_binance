import { randomUUID } from "node:crypto";

import { expect, test, type APIRequestContext } from "@playwright/test";

const AUTH_API_BASE_URL = "http://127.0.0.1:18080";
const ADMIN_EMAIL = "admin@example.com";
const ADMIN_PASSWORD = "pass1234";
const SUPER_ADMIN_EMAIL = "admin-commercial-super@example.com";
const MANUAL_CREDIT_CONFIRMATION = "MANUAL_CREDIT_MEMBERSHIP";

let uniqueCounter = 0;
let operatorAdminSessionToken = "";
let superAdminSessionToken = "";

test.describe("admin commercial", () => {
  test.describe.configure({ mode: "serial" });
  test.beforeAll(async ({ request }) => {
    operatorAdminSessionToken = await createAdminSession(request, ADMIN_EMAIL);
    superAdminSessionToken = await createAdminSession(request, SUPER_ADMIN_EMAIL);
  });

  test("operator admin sees restricted commercial controls but can process abnormal orders and runtime review", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = operatorAdminSessionToken;
    const viewerEmail = uniqueEmail("viewer");
    const creditEmail = uniqueEmail("credit");
    const orphanEmail = uniqueEmail("orphan");
    const rejectEmail = uniqueEmail("reject");
    const strategyName = `admin-draft-${Date.now()}`;

    await createVerifiedUserSession(request, viewerEmail, "pass1234");
    const seeded = await seedAdminCommercialData(request, adminSessionToken, {
      creditEmail,
      orphanEmail,
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
    await expect(page.getByRole("heading", { name: "Operations Overview" })).toBeVisible();
    await expect(page.getByText("Operator Boundary", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Operator boundary is active", { exact: false }).first()).toBeVisible();
    await expect(page.getByRole("link", { name: "Audit" })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Templates" })).toHaveCount(0);

    await page.goto("/admin/users");
    await expect(page.getByRole("heading", { name: "User Management" })).toBeVisible();
    await expect(page.getByText(viewerEmail, { exact: true })).toBeVisible();
    await expect(page.getByText("No membership", { exact: false }).first()).toBeVisible();
    await expectForbiddenAdminRead(request, adminSessionToken, "/admin/audit");

    await page.goto("/admin/memberships");
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await expect(page.getByText("Price Matrix", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("super_admin session is required", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open Membership" })).toHaveCount(0);
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/memberships/plans", {
      code: "monthly",
      name: "Monthly",
      duration_days: 30,
      is_active: true,
      prices: [{ chain: "BSC", asset: "USDT", amount: "20.00" }],
    });
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/memberships/manage", {
      email: uniqueEmail("blocked-member"),
      action: "open",
      duration_days: 30,
      at: new Date().toISOString(),
    });

    await page.goto("/admin/deposits");
    await expect(page.getByRole("heading", { name: "Abnormal Deposit Handling" })).toBeVisible();
    await expect(page.getByText(seeded.rejectTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText(seeded.creditTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText(seeded.orphanTxHash, { exact: true })).toBeVisible();
    await expect(page.getByText("Target Order Snapshot", { exact: false })).toBeVisible();
    const rejectRow = page.getByRole("row", { name: new RegExp(seeded.rejectTxHash) });
    await rejectRow.getByRole("button", { name: "Reject Deposit" }).click();
    await expect(page.getByText("Result: manual_rejected", { exact: false })).toBeVisible();
    const creditRow = page.getByRole("row", { name: new RegExp(seeded.creditTxHash) });
    await expect(creditRow.getByText(MANUAL_CREDIT_CONFIRMATION, { exact: false })).toBeVisible();
    await expect(creditRow.getByText("Wrong asset transfer", { exact: false })).toBeVisible();
    await creditRow.getByLabel("Confirmation Phrase").fill(MANUAL_CREDIT_CONFIRMATION);
    await creditRow
      .getByLabel("Review Notes")
      .fill("operator reviewed wrong-asset transfer and validated order ownership");
    await creditRow.getByRole("button", { name: "Manual Credit" }).click();
    await expect(page.getByText("Result: manual_approved", { exact: false })).toBeVisible();
    await expect(page.getByRole("row", { name: new RegExp(`${seeded.creditTxHash}.*manual_approved`) })).toBeVisible();
    const orphanRow = page.getByRole("row", { name: new RegExp(seeded.orphanTxHash) });
    await expect(orphanRow.getByLabel("Target Order")).toBeVisible();
    await expect(orphanRow.getByRole("button", { name: "Manual Credit" })).toBeDisabled();

    await page.goto("/admin/address-pools");
    await expect(page.getByRole("heading", { name: "Address Pool Inventory" })).toBeVisible();
    await expect(page.getByText("bsc-addr-1", { exact: true })).toBeVisible();
    await expect(page.getByText("Enabled inventory", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Add or Enable Address" })).toHaveCount(0);
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/address-pools", {
      chain: "BSC",
      address: `blocked-bsc-${Date.now()}`,
      is_enabled: true,
    });

    await page.goto("/admin/templates");
    await expect(page).toHaveURL(/\/admin\/dashboard$/);
    await expect(page.getByRole("heading", { name: "Operations Overview" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Template Management" })).toHaveCount(0);

    await expectForbiddenAdminRead(request, adminSessionToken, "/admin/templates");
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/templates", {
      name: "Blocked Template",
      symbol: "ADAUSDT",
      market: "Spot",
      mode: "SpotClassic",
      generation: "Custom",
      levels: [
        { entry_price: "1.00", quantity: "10", take_profit_bps: 150, trailing_bps: null },
        { entry_price: "1.10", quantity: "10", take_profit_bps: 180, trailing_bps: null },
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
      overall_take_profit_bps: 350,
      overall_stop_loss_bps: 120,
      post_trigger_action: "Rebuild",
    });

    await page.goto("/admin/strategies");
    await expect(page.getByRole("heading", { name: "Strategy Oversight" })).toBeVisible();
    await expect(page.getByText(strategyName, { exact: true })).toBeVisible();
    await expect(page.getByText("Selected Detail", { exact: false })).toBeVisible();
    await expect(page.getByRole("columnheader", { name: "Pre-flight" })).toBeVisible();
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/sweeps", {
      chain: "SOL",
      asset: "USDC",
      treasury_address: "blocked-sol-treasury",
      requested_at: new Date().toISOString(),
      transfers: [{ from_address: "sol-addr-1", amount: "12.50000000" }],
    });

    await page.goto("/admin/system");
    await expect(page.getByRole("heading", { name: "System Settings" })).toBeVisible();
    await expect(page.getByText("cannot change confirmation counts", { exact: false })).toBeVisible();
    await expect(page.getByLabel("ETH Confirmations")).toBeDisabled();
    await expect(page.getByLabel("BSC Confirmations")).toBeDisabled();
    await expect(page.getByLabel("SOL Confirmations")).toBeDisabled();
    await expect(page.getByRole("button", { name: "Save Confirmation Policy" })).toBeDisabled();
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/system", {
      eth_confirmations: 18,
      bsc_confirmations: 15,
      sol_confirmations: 22,
    });


  });

  test("super admin manages pricing, memberships, templates, inventory, sweeps, and audit-backed system controls", async ({
    context,
    page,
    request,
  }) => {
    const adminSessionToken = superAdminSessionToken;
    const memberEmail = uniqueEmail("member");
    const templateUserEmail = uniqueEmail("template-user");
    const extraAddress = `bsc-ops-${Date.now()}`;
    const templateName = `ADA Trend Rider ${Date.now()}`;
    const appliedStrategyName = `ADA Rider Copy ${Date.now()}`;

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
    await expect(page.getByText("Super Admin", { exact: false }).first()).toBeVisible();
    await expect(page.getByRole("link", { name: "Audit" })).toHaveCount(1);
    await expect(page.getByRole("link", { name: "Templates" })).toHaveCount(1);

    await page.goto("/admin/memberships");
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    const planForm = page.locator('form[action="/api/admin/memberships"]').filter({ has: page.getByLabel("Plan Code") }).first();
    const memberForm = page.locator('form[action="/api/admin/memberships"]').filter({ has: page.getByLabel("Member Email") }).first();
    await page.getByLabel("Selected Plan").selectOption("quarterly");
    await page.getByRole("button", { name: "Load Plan" }).click();
    await expect(planForm.getByLabel("Plan Code")).toHaveValue("quarterly");
    await planForm.getByLabel("Display Name").fill("Quarterly");
    await planForm.getByLabel("Duration Days").fill("91");
    await planForm.getByLabel("ETH / USDT Price").fill("58.80");
    await planForm.getByLabel("ETH / USDC Price").fill("58.70");
    await planForm.getByLabel("BSC / USDT Price").fill("58.50");
    await planForm.getByLabel("BSC / USDC Price").fill("58.40");
    await planForm.getByLabel("SOL / USDT Price").fill("58.20");
    await planForm.getByLabel("SOL / USDC Price").fill("58.10");
    await planForm.getByRole("button", { name: "Save Price Matrix" }).click();
    await expect(page.getByText("Price Matrix Saved", { exact: false })).toBeVisible();
    await expect(page.getByText("Saved plan: quarterly", { exact: false })).toBeVisible();
    await expect(page.getByText("58.50", { exact: false })).toBeVisible();
    await expect(page.getByText("58.70", { exact: false })).toBeVisible();
    await expect(page.getByText("58.20", { exact: false })).toBeVisible();
    await memberForm.getByLabel("Member Email").fill(memberEmail);
    await memberForm.getByLabel("Duration Days").fill("30");
    await memberForm.getByRole("button", { name: "Open Membership" }).click();
    await expect(page.getByText(memberEmail, { exact: false }).first()).toBeVisible();
    await expect(page.getByText("Membership Change Recorded", { exact: false })).toBeVisible();
    const membershipRow = page.getByRole("row", { name: new RegExp(memberEmail) });
    await expect(membershipRow).toBeVisible();
    await membershipRow.getByLabel("Extend Days").fill("15");
    await membershipRow.getByRole("button", { name: "Extend Membership" }).click();
    await expect(page.getByText("Membership Change Recorded", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Freeze" }).click();
    await expect(membershipRow.getByText("Frozen", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Unfreeze" }).click();
    await expect(page.getByText("Membership Change Recorded", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Revoke" }).click();
    await expect(membershipRow.getByText("Revoked", { exact: false })).toBeVisible();

    await page.goto("/admin/address-pools");
    await expect(page.getByRole("heading", { name: "Address Pool Inventory" })).toBeVisible();
    const baseAddressRow = page.getByRole("row", { name: /bsc-addr-1/ });
    await baseAddressRow.getByRole("button", { name: "Disable Address" }).click();
    await expect(page.getByText("Address Pool Updated", { exact: false })).toBeVisible();
    await page.getByLabel("Chain Allocation").selectOption("BSC");
    await page.getByLabel("Address").fill(extraAddress);
    await page.getByRole("button", { name: "Add or Enable Address" }).click();
    await expect(page.getByText(extraAddress, { exact: true })).toBeVisible();

    await createTemplate(request, adminSessionToken, templateName);
    await page.goto("/admin/templates");
    await expect(page.getByRole("heading", { name: "Template Management" })).toBeVisible();
    await expect(page.getByText(templateName, { exact: true })).toBeVisible();
    const templateUserToken = await createVerifiedUserSession(request, templateUserEmail, "pass1234");
    const createdTemplate = await findTemplateByName(request, adminSessionToken, templateName);
    const applied = await applyTemplate(request, templateUserToken, createdTemplate.id, appliedStrategyName);
    expect(applied.name).toBe(appliedStrategyName);
    expect(applied.symbol).toBe("ADAUSDT");
    expect(applied.market).toBe("Spot");
    expect(applied.mode).toBe("SpotClassic");
    expect(applied.source_template_id).toBe(createdTemplate.id);
    expect(applied.draft_revision.levels.length).toBeGreaterThan(0);

    await createSweepJob(request, adminSessionToken, extraAddress);
    await page.goto("/admin/sweeps");
    await expect(page.getByRole("heading", { name: "Sweep Operations" })).toBeVisible();

    await page.goto("/admin/system");
    await expect(page.getByRole("heading", { name: "System Settings" })).toBeVisible();
    await page.getByLabel("ETH Confirmations").fill("18");
    await page.getByLabel("BSC Confirmations").fill("15");
    await page.getByLabel("SOL Confirmations").fill("22");
    await page.getByRole("button", { name: "Save Confirmation Policy" }).click();
    await expect(page.getByText("Confirmation Policy Saved", { exact: false })).toBeVisible();
    await expect(page.getByText("ETH 18", { exact: false })).toBeVisible();
    await expect(page.getByText("BSC 15", { exact: false })).toBeVisible();
    await expect(page.getByText("SOL 22", { exact: false })).toBeVisible();

    await page.goto("/admin/audit");
    await expect(page.getByRole("heading", { name: "Audit Review" })).toBeVisible();
    await expect(page.getByText(templateName, { exact: false }).first()).toBeVisible();
    await expect(page.getByText(memberEmail, { exact: false }).first()).toBeVisible();
    await expect(page.getByText("treasury-bsc-main", { exact: false }).first()).toBeVisible();
  });
  test("web admin bridge preserves backend 403 and 400 semantics", async ({ request }) => {
    const operatorToken = operatorAdminSessionToken;
    const superAdminToken = superAdminSessionToken;

    const forbiddenResponse = await request.post("http://localhost:13000/api/admin/system", {
      failOnStatusCode: false,
      form: {
        ethConfirmations: "18",
        bscConfirmations: "15",
        solConfirmations: "22",
      },
      headers: {
        cookie: `session_token=${operatorToken}`,
      },
    });
    expect(forbiddenResponse.status()).toBe(403);
    expect(await forbiddenResponse.text()).toContain("super admin access required");

    const badRequestResponse = await request.post("http://localhost:13000/api/admin/system", {
      failOnStatusCode: false,
      form: {
        ethConfirmations: "0",
        bscConfirmations: "15",
        solConfirmations: "22",
      },
      headers: {
        cookie: `session_token=${superAdminToken}`,
      },
    });
    expect(badRequestResponse.status()).toBe(400);
    expect(await badRequestResponse.text()).toContain("eth_confirmations must be greater than 0");
  });


});

async function seedAdminCommercialData(
  request: APIRequestContext,
  adminSessionToken: string,
  input: { creditEmail: string; orphanEmail: string; rejectEmail: string; strategyName: string },
) {
  const creditUserToken = await createVerifiedUserSession(request, input.creditEmail, "pass1234");
  const orphanUserToken = await createVerifiedUserSession(request, input.orphanEmail, "pass1234");
  const rejectUserToken = await createVerifiedUserSession(request, input.rejectEmail, "pass1234");

  const creditOrder = await createBillingOrder(request, creditUserToken, input.creditEmail);
  const orphanOrder = await createBillingOrder(request, orphanUserToken, input.orphanEmail);
  const rejectOrder = await createBillingOrder(request, rejectUserToken, input.rejectEmail);

  const creditTxHash = `tx-credit-${Date.now()}`;
  const orphanTxHash = `tx-orphan-${Date.now()}`;
  const rejectTxHash = `tx-reject-${Date.now()}`;

  await matchOrderAsAbnormal(request, adminSessionToken, {
    address: creditOrder.address,
    amount: creditOrder.amount,
    asset: "USDC",
    chain: "BSC",
    txHash: creditTxHash,
  });
  await matchOrderAsAbnormal(request, adminSessionToken, {
    address: `unknown-${Date.now()}`,
    amount: orphanOrder.amount,
    asset: "USDT",
    chain: "BSC",
    txHash: orphanTxHash,
  });
  await matchOrderAsAbnormal(request, adminSessionToken, {
    address: rejectOrder.address,
    amount: "19.50000000",
    asset: "USDT",
    chain: "BSC",
    txHash: rejectTxHash,
  });

  await createStrategy(request, creditUserToken, input.strategyName);

  return {
    creditOrderId: creditOrder.orderId,
    creditTxHash,
    orphanOrderId: orphanOrder.orderId,
    orphanTxHash,
    rejectTxHash,
  };
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

async function createVerifiedUserSession(request: APIRequestContext, email: string, password: string) {
  await ensureVerifiedUser(request, email, password);
  return login(request, email, password);
}

async function ensureVerifiedUser(request: APIRequestContext, email: string, password: string) {
  const register = await request.post(`${AUTH_API_BASE_URL}/auth/register`, {
    data: { email, password },
  });

  if (register.ok()) {
    const payload = (await register.json()) as { user_id?: number };
    expect(typeof payload.user_id).toBe("number");
    return;
  }

  const body = await register.text();
  if (register.status() === 409 || body.includes("user already exists")) {
    return;
  }

  throw new Error(`ensureVerifiedUser failed ${email} ${register.status()} ${body}`);
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

async function createSweepJob(
  request: APIRequestContext,
  adminSessionToken: string,
  fromAddress = "bsc-addr-1",
) {
  const response = await request.post(`${AUTH_API_BASE_URL}/admin/sweeps`, {
    data: {
      chain: "BSC",
      asset: "USDT",
      treasury_address: "treasury-bsc-main",
      requested_at: new Date().toISOString(),
      transfers: [{ from_address: fromAddress, amount: "20.00000000" }],
    },
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok(), await response.text()).toBeTruthy();
}

async function createTemplate(
  request: APIRequestContext,
  adminSessionToken: string,
  templateName: string,
) {
  const response = await request.post(`${AUTH_API_BASE_URL}/admin/templates`, {
    data: {
      name: templateName,
      symbol: "ADAUSDT",
      market: "Spot",
      mode: "SpotClassic",
      generation: "Geometric",
      levels: [
        { entry_price: "0.9500", quantity: "120", take_profit_bps: 140, trailing_bps: 80 },
        { entry_price: "1.0500", quantity: "130", take_profit_bps: 170, trailing_bps: 90 },
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
      overall_take_profit_bps: 420,
      overall_stop_loss_bps: 160,
      post_trigger_action: "Rebuild",
    },
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.ok()).toBeTruthy();
}

async function expectForbiddenAdminWrite(
  request: APIRequestContext,
  sessionToken: string,
  path: string,
  data: Record<string, unknown>,
) {
  const response = await request.post(`${AUTH_API_BASE_URL}${path}`, {
    data,
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
  });
  expect(response.status()).toBe(403);
  const body = await response.text();
  expect(body).toContain("super admin access required");
}

async function expectForbiddenAdminRead(
  request: APIRequestContext,
  sessionToken: string,
  path: string,
) {
  const response = await request.get(`${AUTH_API_BASE_URL}${path}`, {
    headers: {
      authorization: `Bearer ${sessionToken}`,
    },
  });
  expect(response.status()).toBe(403);
  const body = await response.text();
  expect(body).toContain("super admin access required");
}

async function findTemplateByName(
  request: APIRequestContext,
  adminSessionToken: string,
  name: string,
) {
  const response = await request.get(`${AUTH_API_BASE_URL}/admin/templates`, {
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
    },
  });
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as {
    items?: Array<{ id: string; name: string }>;
  };
  const match = payload.items?.find((item) => item.name === name);
  expect(match).toBeTruthy();
  return match!;
}

async function getTemplateById(
  request: APIRequestContext,
  adminSessionToken: string,
  templateId: string,
) {
  const response = await request.get(`${AUTH_API_BASE_URL}/admin/templates`, {
    headers: {
      authorization: `Bearer ${adminSessionToken}`,
    },
  });
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as {
    items?: Array<{
      generation: string;
      id: string;
      levels: Array<{
        entry_price: string;
        quantity: string;
        take_profit_bps: number;
        trailing_bps: number | null;
      }>;
      name: string;
      overall_stop_loss_bps: number | null;
      overall_take_profit_bps: number | null;
      post_trigger_action: string;
      symbol: string;
    }>;
  };
  const match = payload.items?.find((item) => item.id === templateId);
  expect(match).toBeTruthy();
  return match!;
}

async function applyTemplate(
  request: APIRequestContext,
  sessionToken: string,
  templateId: string,
  name: string,
) {
  const response = await request.post(`${AUTH_API_BASE_URL}/admin/templates/${templateId}/apply`, {
    data: { name },
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
  });
  if (!response.ok()) {
    throw new Error(`applyTemplate failed ${response.status()} ${await response.text()}`);
  }
  return (await response.json()) as {
    draft_revision: {
      generation: string;
      levels: Array<{
        entry_price: string;
        quantity: string;
        take_profit_bps: number;
        trailing_bps: number | null;
      }>;
      overall_stop_loss_bps: number | null;
      overall_take_profit_bps: number | null;
      post_trigger_action: string;
    };
    market: string;
    mode: string;
    name: string;
    source_template_id: string | null;
    symbol: string;
  };
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
  return `${prefix}-${Date.now()}-${uniqueCounter}-${randomUUID()}@example.com`;
}
