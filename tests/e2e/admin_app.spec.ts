import { expect, test } from "@playwright/test";

test("admin can manage members and address pools", async ({ page }) => {
  await page.goto("/admin/dashboard");
  await expect(page.locator("main")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Admin Dashboard" })).toBeVisible();
  await expect(page.getByText("Price config review")).toBeVisible();
  await expect(page.getByRole("link", { name: "Member Control" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Address Pools" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Templates" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Billing Admin" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Audit Logs" })).toBeVisible();

  await page.getByRole("link", { name: "Member Control" }).click();
  await expect(page).toHaveURL(/\/admin\/users$/);
  await expect(page.getByRole("heading", { name: "Member Control" })).toBeVisible();
  await expect(page.getByText("membership overrides")).toBeVisible();

  await page.getByRole("link", { name: "Address Pools" }).click();
  await expect(page).toHaveURL(/\/admin\/address-pools$/);
  await expect(page.getByRole("heading", { name: "Address Pool Expansion" })).toBeVisible();
  await expect(page.getByText("Treasury sweep queue")).toBeVisible();

  await page.goto("/admin/dashboard");
  await page.getByRole("link", { name: "Templates" }).click();
  await expect(page).toHaveURL(/\/admin\/templates$/);
  await expect(page.getByRole("heading", { name: "Admin Templates" })).toBeVisible();

  await page.goto("/admin/dashboard");
  await page.getByRole("link", { name: "Billing Admin" }).click();
  await expect(page).toHaveURL(/\/admin\/billing$/);
  await expect(page.getByRole("heading", { name: "Billing Admin" })).toBeVisible();

  await page.goto("/admin/dashboard");
  await page.getByRole("link", { name: "Audit Logs" }).click();
  await expect(page).toHaveURL(/\/admin\/audit$/);
  await expect(page.getByRole("heading", { name: "Audit Logs" })).toBeVisible();
  await expect(page.getByText("Treasury sweep views")).toBeVisible();
});
