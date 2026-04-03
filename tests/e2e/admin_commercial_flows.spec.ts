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
    const viewerEmail = uniqueEmail("viewer");
    const creditEmail = uniqueEmail("credit");
    const rejectEmail = uniqueEmail("reject");
    const strategyName = `admin-draft-${Date.now()}`;

    await createVerifiedUserSession(request, viewerEmail, "pass1234");
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
    await expect(page.getByRole("link", { name: "Audit" })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Templates" })).toHaveCount(0);

    await page.getByRole("link", { name: "Users" }).click();
    await expect(page.getByRole("heading", { name: "User Management" })).toBeVisible();
    await expect(page.getByText("Identity-backed user directory", { exact: false })).toBeVisible();
    await expect(page.getByText(viewerEmail, { exact: true })).toBeVisible();
    await expect(page.getByText("No membership record", { exact: false })).toBeVisible();
    await expectForbiddenAdminRead(request, adminSessionToken, "/admin/audit");

    await page.getByRole("link", { name: "Memberships" }).click();
    await expect(page.getByRole("heading", { name: "Membership Operations" })).toBeVisible();
    await expect(page.getByText("Plan & pricing management", { exact: false })).toBeVisible();
    await expect(page.getByText("super_admin required", { exact: false })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open membership" })).not.toBeVisible();
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
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/address-pools", {
      chain: "BSC",
      address: `blocked-bsc-${Date.now()}`,
      is_enabled: true,
    });

    await page.goto("/admin/templates");
    await expect(page).toHaveURL(/\/admin\/dashboard$/);
    await expect(page.getByRole("heading", { name: "Admin Dashboard" })).toBeVisible();
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

    await page.getByRole("link", { name: "Strategies" }).click();
    await expect(page.getByRole("heading", { name: "Strategy Oversight" })).toBeVisible();
    await expect(page.getByText("Runtime overview", { exact: false })).toBeVisible();
    await expect(page.getByText(strategyName, { exact: true })).toBeVisible();
    await expect(page.getByText("Active orders", { exact: false })).toBeVisible();
    await expect(page.getByText("Last pre-flight", { exact: false })).toBeVisible();
    await expectForbiddenAdminWrite(request, adminSessionToken, "/admin/sweeps", {
      chain: "SOL",
      asset: "USDC",
      treasury_address: "blocked-sol-treasury",
      requested_at: new Date().toISOString(),
      transfers: [{ from_address: "sol-addr-1", amount: "12.50000000" }],
    });

    await page.getByRole("link", { name: "System" }).click();
    await expect(page.getByRole("heading", { name: "System Configuration" })).toBeVisible();
    await expect(page.getByText("operator_admin sessions can review but cannot change confirmation counts", { exact: false })).toBeVisible();
    await expect(page.getByLabel("ETH confirmations")).toBeDisabled();
    await expect(page.getByLabel("BSC confirmations")).toBeDisabled();
    await expect(page.getByLabel("SOL confirmations")).toBeDisabled();
    await expect(page.getByRole("button", { name: "Save confirmation policy" })).toBeDisabled();
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
    const adminSessionToken = await createAdminSession(request, SUPER_ADMIN_EMAIL);
    const memberEmail = uniqueEmail("member");
    const creditEmail = uniqueEmail("credit");
    const rejectEmail = uniqueEmail("reject");
    const templateUserEmail = uniqueEmail("template-user");
    const strategyName = `admin-draft-${Date.now()}`;
    const extraAddress = `bsc-ops-${Date.now()}`;
    const templateName = `ADA Trend Rider ${Date.now()}`;
    const appliedStrategyName = `ADA Rider Copy ${Date.now()}`;

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
    await page.getByLabel("ETH / USDT price").fill("24.80");
    await page.getByLabel("ETH / USDC price").fill("24.70");
    await page.getByLabel("BSC / USDT price").fill("24.50");
    await page.getByLabel("BSC / USDC price").fill("24.40");
    await page.getByLabel("SOL / USDT price").fill("24.20");
    await page.getByLabel("SOL / USDC price").fill("24.10");
    await page.getByRole("button", { name: "Save plan pricing" }).click();
    await expect(page.getByText("Plan pricing saved", { exact: false })).toBeVisible();
    await expect(page.getByText("24.50", { exact: false })).toBeVisible();
    await expect(page.getByText("24.70", { exact: false })).toBeVisible();
    await expect(page.getByText("24.20", { exact: false })).toBeVisible();
    await page.getByLabel("Member email").fill(memberEmail);
    await page.getByLabel("Membership duration days").fill("30");
    await page.getByRole("button", { name: "Open membership" }).click();
    await expect(page.getByText(`Target: ${memberEmail}`, { exact: false })).toBeVisible();
    await expect(page.getByText("Status: Active", { exact: false })).toBeVisible();
    const membershipRow = page.getByRole("row", { name: new RegExp(memberEmail) });
    await expect(membershipRow).toBeVisible();
    await membershipRow.getByLabel("Membership duration days").fill("15");
    await membershipRow.getByRole("button", { name: "Extend membership" }).click();
    await expect(page.getByText("Last action: extend", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Freeze membership" }).click();
    await expect(page.getByText("Status: Frozen", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Unfreeze membership" }).click();
    await expect(page.getByText("Last action: unfreeze", { exact: false })).toBeVisible();
    await membershipRow.getByRole("button", { name: "Revoke membership" }).click();
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
    await page.getByLabel("Template name").fill(templateName);
    await page.getByRole("textbox", { name: "Symbol" }).fill("ADAUSDT");
    await page.locator('select[name="market"]').selectOption("Spot");
    await page.locator('select[name="mode"]').selectOption("SpotClassic");
    await page.locator('select[name="generation"]').selectOption("Geometric");
    await page.getByLabel("Level 1 entry price").fill("0.9500");
    await page.getByLabel("Level 1 quantity").fill("120");
    await page.getByLabel("Level 1 take profit (bps)").fill("140");
    await page.getByLabel("Level 1 trailing (bps)").fill("80");
    await page.getByLabel("Level 2 entry price").fill("1.0500");
    await page.getByLabel("Level 2 quantity").fill("130");
    await page.getByLabel("Level 2 take profit (bps)").fill("170");
    await page.getByLabel("Level 2 trailing (bps)").fill("90");
    await page.getByLabel("Membership ready").selectOption("true");
    await page.getByLabel("Exchange ready").selectOption("true");
    await page.getByLabel("Permissions ready").selectOption("true");
    await page.getByLabel("Withdrawals disabled").selectOption("true");
    await page.getByLabel("Hedge mode ready").selectOption("true");
    await page.getByLabel("Symbol ready").selectOption("true");
    await page.getByLabel("Filters ready").selectOption("true");
    await page.getByLabel("Margin ready").selectOption("true");
    await page.getByLabel("Conflict ready").selectOption("true");
    await page.getByLabel("Balance ready").selectOption("true");
    await page.getByLabel("Overall take profit (bps)").fill("420");
    await page.getByLabel("Overall stop loss (bps)").fill("160");
    await page.getByLabel("Post-trigger action").selectOption("Rebuild");
    await page.getByRole("button", { name: "Create template" }).click();
    await expect(page.getByText("Template created", { exact: false })).toBeVisible();
    await expect(page.getByText(templateName, { exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "Geometric" }).first()).toBeVisible();
    await expect(page.getByText("2 levels", { exact: false })).toBeVisible();
    const templateUserToken = await createVerifiedUserSession(request, templateUserEmail, "pass1234");
    const createdTemplate = await findTemplateByName(request, adminSessionToken, templateName);
    const applied = await applyTemplate(request, templateUserToken, createdTemplate.id, appliedStrategyName);
    expect(applied.name).toBe(appliedStrategyName);
    expect(applied.symbol).toBe("ADAUSDT");
    expect(applied.market).toBe("Spot");
    expect(applied.mode).toBe("SpotClassic");
    expect(applied.source_template_id).toBe(createdTemplate.id);
    expect(applied.draft_revision.generation).toBe("Geometric");
    expect(applied.draft_revision.levels).toHaveLength(2);
    expect(applied.draft_revision.levels[0]?.entry_price).toBe("0.9500");
    expect(applied.draft_revision.levels[0]?.quantity).toBe("120");
    expect(applied.draft_revision.levels[0]?.take_profit_bps).toBe(140);
    expect(applied.draft_revision.levels[0]?.trailing_bps).toBe(80);
    expect(applied.draft_revision.levels[1]?.entry_price).toBe("1.0500");
    expect(applied.draft_revision.levels[1]?.quantity).toBe("130");
    expect(applied.draft_revision.overall_take_profit_bps).toBe(420);
    expect(applied.draft_revision.overall_stop_loss_bps).toBe(160);
    expect(applied.draft_revision.post_trigger_action).toBe("Rebuild");
    await page.getByRole("button", { name: `Edit ${templateName}` }).click();
    await expect(page.getByRole("heading", { name: "Edit template" })).toBeVisible();
    await page.getByLabel("Template name").fill(`${templateName} v2`);
    await page.getByRole("textbox", { name: "Symbol" }).fill("XRPUSDT");
    await page.locator('select[name="generation"]').selectOption("Arithmetic");
    await page.getByLabel("Level 1 entry price").fill("0.5000");
    await page.getByLabel("Level 1 quantity").fill("150");
    await page.getByLabel("Level 1 take profit (bps)").fill("125");
    await page.getByLabel("Level 1 trailing (bps)").fill("60");
    await page.getByLabel("Level 2 entry price").fill("0.6500");
    await page.getByLabel("Level 2 quantity").fill("180");
    await page.getByLabel("Level 2 take profit (bps)").fill("155");
    await page.getByLabel("Level 2 trailing (bps)").fill("70");
    await page.getByLabel("Overall take profit (bps)").fill("390");
    await page.getByLabel("Overall stop loss (bps)").fill("145");
    await page.getByLabel("Post-trigger action").selectOption("Stop");
    await page.getByRole("button", { name: "Save template changes" }).click();
    await expect(page.getByText("Template updated", { exact: false })).toBeVisible();
    await expect(page.getByText(`${templateName} v2`, { exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "Arithmetic" }).first()).toBeVisible();
    const updatedTemplate = await getTemplateById(request, adminSessionToken, createdTemplate.id);
    expect(updatedTemplate.name).toBe(`${templateName} v2`);
    expect(updatedTemplate.symbol).toBe("XRPUSDT");
    expect(updatedTemplate.generation).toBe("Arithmetic");
    expect(updatedTemplate.levels[0]?.entry_price).toBe("0.5000");
    expect(updatedTemplate.levels[0]?.quantity).toBe("150");
    expect(updatedTemplate.overall_take_profit_bps).toBe(390);
    expect(updatedTemplate.overall_stop_loss_bps).toBe(145);
    expect(updatedTemplate.post_trigger_action).toBe("Stop");
    const appliedAfterUpdate = await applyTemplate(
      request,
      templateUserToken,
      createdTemplate.id,
      `${appliedStrategyName} v2`,
    );
    expect(appliedAfterUpdate.symbol).toBe("XRPUSDT");
    expect(appliedAfterUpdate.draft_revision.generation).toBe("Arithmetic");
    expect(appliedAfterUpdate.draft_revision.levels[0]?.entry_price).toBe("0.5000");
    expect(appliedAfterUpdate.draft_revision.overall_take_profit_bps).toBe(390);
    expect(appliedAfterUpdate.draft_revision.post_trigger_action).toBe("Stop");
    expect(applied.symbol).toBe("ADAUSDT");
    expect(applied.draft_revision.generation).toBe("Geometric");
    expect(applied.draft_revision.levels[0]?.entry_price).toBe("0.9500");
    expect(applied.draft_revision.overall_take_profit_bps).toBe(420);

    await page.getByRole("link", { name: "Sweeps" }).click();
    await expect(page.getByRole("heading", { name: "Sweep Operations" })).toBeVisible();
    await page.locator('select[name="chain"]').selectOption("SOL");
    await page.locator('select[name="asset"]').selectOption("USDC");
    await page.getByLabel("Treasury address").fill("treasury-sol-main");
    await page.getByLabel("Source address").fill("sol-addr-2");
    await page.getByLabel("Sweep amount").fill("18.50000000");
    await page.getByRole("button", { name: "Request sweep" }).click();
    await expect(page.getByText("Sweep request submitted", { exact: false })).toBeVisible();
    await expect(page.getByRole("cell", { name: "treasury-sol-main" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "SOL / USDC" }).first()).toBeVisible();

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
    await expect(page.getByRole("cell", { name: "strategy.template_updated" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "membership.plan_config_updated" }).first()).toBeVisible();
    await expect(page.getByRole("cell", { name: "treasury.sweep_requested" }).first()).toBeVisible();
    await expect(page.getByText("session role super_admin", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("session sid", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("before monthly Monthly 30d active true prices BSC/USDC 20.00000000, BSC/USDT 20.00000000, ETH/USDC 20.00000000, ETH/USDT 20.00000000, SOL/USDC 20.00000000, SOL/USDT 20.00000000", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("after monthly Monthly 31d active true prices BSC/USDC 24.40000000, BSC/USDT 24.50000000, ETH/USDC 24.70000000, ETH/USDT 24.80000000, SOL/USDC 24.10000000, SOL/USDT 24.20000000", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("before status Pending | active - | grace - | override none", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("after status Active | active", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("before BSC bsc-addr-1 enabled", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("after BSC bsc-addr-1 disabled", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("before ETH 12 | BSC 12 | SOL 12", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("after ETH 18 | BSC 15 | SOL 22", { exact: false }).first()).toBeVisible();
    await expect(page.getByText("before no prior sweep job | after SOL USDC treasury-sol-main 1 transfer", { exact: false }).first()).toBeVisible();
    await expect(page.getByText(templateName, { exact: false }).first()).toBeVisible();
    await expect(page.getByText("treasury-sol-main", { exact: false }).first()).toBeVisible();
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
  return `${prefix}-${Date.now()}-${uniqueCounter}@example.com`;
}
