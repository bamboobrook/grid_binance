import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable } from "@/components/ui/table";
import { getAdminUsersData } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

function membershipLabel(lang: UiLanguage, status: string | null) {
  switch (status) {
    case "Active":
      return pickText(lang, "有效", "Active");
    case "Grace":
      return pickText(lang, "宽限中", "Grace");
    case "Frozen":
      return pickText(lang, "已冻结", "Frozen");
    case "Revoked":
      return pickText(lang, "已撤销", "Revoked");
    default:
      return pickText(lang, "未开通", "No membership");
  }
}

function registrationLabel(lang: UiLanguage, registered: boolean, verified: boolean) {
  if (registered === false) {
    return pickText(lang, "后台台账", "Ledger only");
  }
  return verified ? pickText(lang, "已注册已验证", "Registered and verified") : pickText(lang, "待邮箱验证", "Verification pending");
}

function orderLabel(lang: UiLanguage, status: string | null) {
  switch (status) {
    case "paid":
      return pickText(lang, "已支付", "Paid");
    case "pending":
      return pickText(lang, "待支付", "Pending");
    case "expired":
      return pickText(lang, "已过期", "Expired");
    case "cancelled":
      return pickText(lang, "已取消", "Cancelled");
    default:
      return status ?? pickText(lang, "无订单", "No order");
  }
}

function roleLabel(lang: UiLanguage, role: string | null) {
  switch (role) {
    case "super_admin":
      return pickText(lang, "超级管理员", "Super Admin");
    case "operator_admin":
      return pickText(lang, "操作员", "Operator Admin");
    default:
      return pickText(lang, "普通用户", "User");
  }
}

export default async function AdminUsersPage() {
  const [cookieStore, data] = await Promise.all([cookies(), getAdminUsersData()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const pendingVerification = data.items.filter((item) => item.registered && item.email_verified === false).length;
  const privilegedUsers = data.items.filter((item) => item.admin_role).length;
  const totpDisabled = data.items.filter((item) => item.registered && item.totp_enabled === false).length;

  return (
    <>
      <AppShellSection
        description={pickText(lang, "把注册状态、会员状态、最近订单与权限角色放进同一张值班台账，便于识别账号异常与权限边界。", "Registration state, membership state, latest order, and role boundary live in one desk ledger for operator triage.")}
        eyebrow={pickText(lang, "用户台账", "User Ledger")}
        title={pickText(lang, "用户管理", "User Management")}
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "待验证邮箱", "Pending Verification")}</CardTitle>
              <CardDescription>{pickText(lang, "注册状态", "Registration State")}</CardDescription>
            </CardHeader>
            <CardBody>{pendingVerification}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "管理员账号", "Privileged Accounts")}</CardTitle>
              <CardDescription>{pickText(lang, "权限角色", "Role Boundary")}</CardDescription>
            </CardHeader>
            <CardBody>{privilegedUsers}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "TOTP 缺口", "TOTP Gap")}</CardTitle>
              <CardDescription>{pickText(lang, "安全状态", "Security State")}</CardDescription>
            </CardHeader>
            <CardBody>{totpDisabled}</CardBody>
          </Card>
        </div>
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "用户主表", "User Ledger Table")}</CardTitle>
              <CardDescription>{pickText(lang, "状态、权限、会员和订单并排展示，避免在多个后台之间跳转确认。", "Status, privilege, membership, and order signals stay side by side.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "email", label: pickText(lang, "用户", "User") },
                  { key: "registration", label: pickText(lang, "注册状态", "Registration State") },
                  { key: "membership", label: pickText(lang, "会员状态", "Membership") },
                  { key: "order", label: pickText(lang, "最近订单", "Latest Order") },
                  { key: "role", label: pickText(lang, "权限角色", "Role Boundary") },
                  { key: "security", label: pickText(lang, "安全", "Security") },
                ]}
                rows={data.items.map((item) => ({
                  email: item.email,
                  id: item.email,
                  membership: membershipLabel(lang, item.membership?.status ?? null),
                  order: orderLabel(lang, item.latest_order_status),
                  registration: registrationLabel(lang, item.registered, item.email_verified),
                  role: roleLabel(lang, item.admin_role),
                  security: item.totp_enabled ? pickText(lang, "已启用 TOTP", "TOTP enabled") : pickText(lang, "未启用 TOTP", "TOTP disabled"),
                }))}
              />
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "值班说明", "Desk Notes")}</CardTitle>
              <CardDescription>{pickText(lang, "明确哪些用户是后台台账、哪些用户仍有安全或验证缺口。", "Make ledger-only records, security gaps, and verification gaps explicit.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "注册状态会区分正式账号与仅有商业台账的记录。", "Registration state separates real accounts from ledger-only records.")}</li>
                <li>{pickText(lang, "权限角色直接暴露 super_admin、operator_admin 与普通用户边界。", "Role boundary keeps super_admin, operator_admin, and regular users explicit.")}</li>
                <li>{pickText(lang, "宽限、冻结、撤销会员会在台账中优先暴露。", "Grace, frozen, and revoked memberships stay visible in the desk ledger.")}</li>
                <li>{pickText(lang, "值班时优先补齐未启用 TOTP 的已注册账号。", "During on-call, prioritize registered accounts that still lack TOTP.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
