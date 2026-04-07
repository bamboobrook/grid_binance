import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import {
  getAdminAddressPoolsData,
  getAdminAuditData,
  getAdminDepositsData,
  getAdminMembershipsData,
  getAdminStrategiesData,
  getCurrentAdminProfile,
  type AdminTemplateList,
  fetchAdminJson,
} from "../../../lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE, type UiLanguage } from "../../../lib/ui/preferences";

function roleBoundaryLabel(lang: UiLanguage, restricted: boolean) {
  return restricted ? pickText(lang, "操作员边界", "Operator Boundary") : pickText(lang, "超级管理员边界", "Super Admin Boundary");
}

function auditActionLabel(lang: UiLanguage, action: string) {
  const map = new Map([
    ["membership_extended", pickText(lang, "会员延长", "Membership Extended")],
    ["membership_frozen", pickText(lang, "会员冻结", "Membership Frozen")],
    ["membership_revoked", pickText(lang, "会员撤销", "Membership Revoked")],
    ["deposit_rejected", pickText(lang, "充值驳回", "Deposit Rejected")],
    ["deposit_credited", pickText(lang, "充值入账", "Deposit Credited")],
    ["template_updated", pickText(lang, "模板更新", "Template Updated")],
    ["system_updated", pickText(lang, "系统配置变更", "System Updated")],
  ]);
  return map.get(action) ?? action;
}

export default async function AdminDashboardPage() {
  const [cookieStore, profile] = await Promise.all([cookies(), getCurrentAdminProfile()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const [memberships, deposits, strategies, audit, templates, pools] = await Promise.all([
    getAdminMembershipsData(),
    getAdminDepositsData(),
    getAdminStrategiesData(),
    profile.admin_role === "super_admin" ? getAdminAuditData() : Promise.resolve({ items: [] }),
    profile.admin_permissions?.can_manage_templates ? fetchAdminJson<AdminTemplateList>("/admin/templates") : Promise.resolve({ items: [] }),
    getAdminAddressPoolsData(),
  ]);

  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipRisk = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const runtimeIncidents = strategies.items.filter((item) => item.status === "ErrorPaused").length;
  const enabledAddresses = pools.addresses.filter((item) => item.is_enabled);
  const enabledByChain = new Map<string, number>();

  for (const item of enabledAddresses) {
    enabledByChain.set(item.chain, (enabledByChain.get(item.chain) ?? 0) + 1);
  }

  const pressuredChains = Array.from(enabledByChain.entries()).filter(([, count]) => count <= 1).map(([chain]) => chain);
  const auditEvents = audit.items.slice(0, 6);
  const role = profile.admin_role ?? "operator_admin";
  const restricted = role === "operator_admin";
  const boardRows = [
    {
      id: "deposits",
      item: pickText(lang, "待处理充值", "Pending Deposits"),
      owner: pickText(lang, "充值审核", "Deposit Review"),
      route: "/admin/deposits",
      severity: openDeposits > 0 ? pickText(lang, "高", "High") : pickText(lang, "低", "Low"),
      summary: pickText(lang, String(openDeposits) + " 笔等待人工复核", String(openDeposits) + " cases await manual review"),
    },
    {
      id: "memberships",
      item: pickText(lang, "会员风险", "Membership Risk"),
      owner: pickText(lang, "会员生命周期", "Membership Lifecycle"),
      route: "/admin/memberships",
      severity: membershipRisk > 0 ? pickText(lang, "中", "Medium") : pickText(lang, "低", "Low"),
      summary: pickText(lang, String(membershipRisk) + " 个账号处于宽限、冻结或撤销", String(membershipRisk) + " accounts need lifecycle follow-up"),
    },
    {
      id: "strategies",
      item: pickText(lang, "异常暂停策略", "Error-paused Strategies"),
      owner: pickText(lang, "策略监督", "Strategy Supervision"),
      route: "/admin/strategies?state=errorpaused",
      severity: runtimeIncidents > 0 ? pickText(lang, "高", "High") : pickText(lang, "低", "Low"),
      summary: pickText(lang, String(runtimeIncidents) + " 个策略被异常暂停", String(runtimeIncidents) + " strategies are blocked by runtime incidents"),
    },
    {
      id: "pools",
      item: pickText(lang, "地址池压力", "Pool Pressure"),
      owner: pickText(lang, "地址池治理", "Address Pool Governance"),
      route: "/admin/address-pools",
      severity: pressuredChains.length > 0 ? pickText(lang, "中", "Medium") : pickText(lang, "低", "Low"),
      summary: pressuredChains.length > 0
        ? pickText(lang, pressuredChains.join("、") + " 仅剩 1 个可用地址", pressuredChains.join(", ") + " are down to one enabled address")
        : pickText(lang, "各链路仍有冗余", "Address redundancy still exists on every chain"),
    },
    {
      id: "audit",
      item: pickText(lang, "审计事件", "Audit Events"),
      owner: pickText(lang, "审计留痕", "Audit Trail"),
      route: restricted ? pickText(lang, "仅超级管理员", "Super Admin Only") : "/admin/audit",
      severity: auditEvents.length > 0 ? pickText(lang, "观察", "Watch") : pickText(lang, "低", "Low"),
      summary: restricted
        ? pickText(lang, "当前会话只能看摘要", "This session can review only the summary")
        : pickText(lang, String(audit.items.length) + " 条最近审计记录", String(audit.items.length) + " recent audit entries"),
    },
  ];

  return (
    <>
      <StatusBanner
        description={restricted
          ? pickText(lang, "当前为操作员值班席位，仅可处理审核与巡检；模板、系统、归集改动仍受权限边界限制。", "Operator on-call boundary is active. Review and supervision stay open, while templates, system, and sweep changes remain restricted.")
          : pickText(lang, "当前为超级管理员值班席位，可直接处理模板、系统配置、归集与审计。", "Super admin on-call boundary is active for templates, system config, sweeps, and audit actions.")}
        title={profile.admin_access_granted ? pickText(lang, "值班权限已生效", "On-call Access Active") : pickText(lang, "值班权限未完成", "On-call Access Pending")}
        tone={profile.admin_access_granted ? "success" : "warning"}
      />
      <AppShellSection
        description={pickText(lang, "围绕待处理充值、会员风险、异常暂停策略、地址池压力与审计事件组织首屏，保持值班席位的处理顺序清晰。", "The first screen is organized around pending deposits, membership risk, error-paused strategies, pool pressure, and audit events so the on-call desk can act in order.")}
        eyebrow={pickText(lang, "值班总览", "On-call Console")}
        title={pickText(lang, "运营总览", "Operations Overview")}
      >
        <div className="content-grid content-grid--metrics">
          <Card tone="accent">
            <CardHeader>
              <CardTitle>{pickText(lang, "待处理充值", "Pending Deposits")}</CardTitle>
              <CardDescription>{pickText(lang, "充值异常队列", "Exception Queue")}</CardDescription>
            </CardHeader>
            <CardBody>{openDeposits}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "会员风险", "Membership Risk")}</CardTitle>
              <CardDescription>{pickText(lang, "宽限、冻结、撤销", "Grace, Frozen, Revoked")}</CardDescription>
            </CardHeader>
            <CardBody>{membershipRisk}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "异常暂停", "Error-paused")}</CardTitle>
              <CardDescription>{pickText(lang, "运行态阻塞", "Runtime Incidents")}</CardDescription>
            </CardHeader>
            <CardBody>{runtimeIncidents}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "地址池压力", "Pool Pressure")}</CardTitle>
              <CardDescription>{pickText(lang, String(enabledAddresses.length) + " / " + String(pools.addresses.length) + " 可用地址", String(enabledAddresses.length) + " of " + String(pools.addresses.length) + " enabled addresses")}</CardDescription>
            </CardHeader>
            <CardBody>{pressuredChains.length}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "审计事件", "Audit Events")}</CardTitle>
              <CardDescription>{restricted ? pickText(lang, "摘要模式", "Summary Only") : pickText(lang, "最近事件", "Recent Events")}</CardDescription>
            </CardHeader>
            <CardBody>{auditEvents.length}</CardBody>
          </Card>
        </div>
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "值班工作板", "On-call Workboard")}</CardTitle>
              <CardDescription>{pickText(lang, "按风险和处理入口排序，避免值班时在页面间来回找线索。", "Sorted by severity and route so the desk does not have to hunt across pages.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "item", label: pickText(lang, "事项", "Item") },
                  { key: "severity", label: pickText(lang, "等级", "Severity") },
                  { key: "summary", label: pickText(lang, "状态摘要", "Summary") },
                  { key: "owner", label: pickText(lang, "责任面板", "Desk") },
                  { key: "route", label: pickText(lang, "入口", "Route") },
                ]}
                rows={boardRows}
              />
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>{pickText(lang, "席位说明", "Desk Boundaries")}</CardTitle>
              <CardDescription>{pickText(lang, "权限边界、模板可见性和地址池冗余一并明确。", "Permission boundary, template visibility, and pool redundancy stay explicit.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "权限角色：" + roleBoundaryLabel(lang, restricted), "Role boundary: " + roleBoundaryLabel(lang, restricted))}</li>
                <li>{pickText(lang, "模板可见数：" + String(templates.items.length) + " 个模板", "Template inventory: " + String(templates.items.length) + " templates visible")}</li>
                <li>{pressuredChains.length > 0 ? pickText(lang, "地址池压力：" + pressuredChains.join("、") + " 需要补仓", "Pool pressure: " + pressuredChains.join(", ") + " require replenishment") : pickText(lang, "地址池压力：暂无", "Pool pressure: none")}</li>
                <li>{restricted ? pickText(lang, "审计可见性：仅摘要", "Audit visibility: summary only") : pickText(lang, "审计可见性：完整事件", "Audit visibility: full event review")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "最近审计事件", "Recent Audit Events")}</CardTitle>
          <CardDescription>{pickText(lang, "保留时间、动作和目标，方便快速回溯最近值班处理。", "Time, action, and target stay visible for quick review of the last operator moves.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "createdAt", label: pickText(lang, "时间", "Timestamp") },
              { key: "action", label: pickText(lang, "动作", "Action") },
              { key: "target", label: pickText(lang, "目标", "Target") },
            ]}
            rows={auditEvents.map((item, index) => ({
              id: item.action + "-" + String(index),
              action: auditActionLabel(lang, item.action),
              createdAt: item.created_at.replace("T", " ").slice(0, 16),
              target: item.target_type + ":" + item.target_id,
            }))}
            emptyMessage={restricted ? pickText(lang, "当前席位只能查看审计摘要，请切换 super_admin 查看明细。", "Summary only in this session. Use a super_admin session for full audit detail.") : pickText(lang, "暂无审计事件。", "No audit events yet.")}
          />
        </CardBody>
      </Card>
    </>
  );
}
