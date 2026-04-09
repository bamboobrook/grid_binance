import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import {
  getAdminAddressPoolsData,
  getAdminAuditData,
  getAdminDepositsData,
  getAdminMembershipsData,
  getAdminStrategiesData,
  getCurrentAdminProfile,
  type AdminTemplateList,
  fetchAdminJson,
} from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

type PageProps = {
  params: Promise<{ locale: string }>;
};

type ActionPanel = {
  id: string;
  actionLabel: string;
  description: string;
  href: string | null;
  title: string;
  value: string;
};

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

function actionButtonClass(disabled: boolean) {
  return disabled
    ? "inline-flex items-center justify-center rounded-lg px-4 py-2 text-sm font-semibold bg-slate-800 text-slate-500 cursor-not-allowed"
    : "inline-flex items-center justify-center rounded-lg px-4 py-2 text-sm font-semibold bg-primary text-primary-foreground hover:bg-primary/90 transition-colors";
}

export default async function AdminDashboardPage({ params }: PageProps) {
  const { locale } = await params;
  const [cookieStore, profile] = await Promise.all([cookies(), getCurrentAdminProfile()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
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

  const pressuredChains = Array.from(enabledByChain.entries())
    .filter(([, count]) => count <= 1)
    .map(([chain]) => chain);
  const auditEvents = audit.items.slice(0, 6);
  const role = profile.admin_role ?? "operator_admin";
  const restricted = role === "operator_admin";
  const panels: ActionPanel[] = [
    {
      id: "deposits",
      title: pickText(lang, "充值审核", "Deposit Review"),
      description: pickText(lang, "人工复核异常转账、手动入账与队列处理。", "Review abnormal transfers, manual credits, and queue handling."),
      value: pickText(lang, String(openDeposits) + " 笔待处理", String(openDeposits) + " pending"),
      href: "/" + locale + "/admin/deposits",
      actionLabel: openDeposits > 0 ? pickText(lang, "立即处理", "Review now") : pickText(lang, "查看详情", "View details"),
    },
    {
      id: "memberships",
      title: pickText(lang, "会员管理", "Memberships"),
      description: pickText(lang, "调整套餐、查看宽限期与冻结状态。", "Adjust plans, review grace windows, and manage freezes."),
      value: pickText(lang, String(membershipRisk) + " 个风险账号", String(membershipRisk) + " accounts at risk"),
      href: "/" + locale + "/admin/memberships",
      actionLabel: pickText(lang, "查看详情", "View details"),
    },
    {
      id: "strategies",
      title: pickText(lang, "策略监督", "Strategies"),
      description: pickText(lang, "查看异常暂停、运行态事件和最近下单。", "Inspect error-paused strategies, runtime events, and recent orders."),
      value: pickText(lang, String(runtimeIncidents) + " 个异常暂停", String(runtimeIncidents) + " paused by incidents"),
      href: "/" + locale + "/admin/strategies",
      actionLabel: runtimeIncidents > 0 ? pickText(lang, "立即处理", "Review now") : pickText(lang, "查看详情", "View details"),
    },
    {
      id: "address-pools",
      title: pickText(lang, "地址池", "Address Pools"),
      description: pickText(lang, "查看各链路可用地址数量与补池压力。", "Review pool capacity and replenishment pressure by chain."),
      value: pressuredChains.length > 0
        ? pickText(lang, pressuredChains.join("、") + " 需要补池", pressuredChains.join(", ") + " need replenishment")
        : pickText(lang, "当前冗余充足", "Healthy redundancy"),
      href: "/" + locale + "/admin/address-pools",
      actionLabel: pickText(lang, "查看详情", "View details"),
    },
  ];

  if (profile.admin_permissions?.can_manage_templates) {
    panels.push({
      id: "templates",
      title: pickText(lang, "模板治理", "Templates"),
      description: pickText(lang, "维护预设参数模板并投放给用户。", "Maintain template presets and publish them to users."),
      value: pickText(lang, String(templates.items.length) + " 个模板", String(templates.items.length) + " templates"),
      href: "/" + locale + "/admin/templates",
      actionLabel: pickText(lang, "查看详情", "View details"),
    });
  }

  panels.push({
    id: "audit",
    title: pickText(lang, "审计记录", "Audit Trail"),
    description: restricted
      ? pickText(lang, "当前会话不是超级管理员，只展示摘要。", "This session is not Super Admin, so only the summary is visible.")
      : pickText(lang, "查看完整审计事件与变更痕迹。", "Review full audit events and change history."),
    value: restricted
      ? pickText(lang, "摘要模式", "Summary only")
      : pickText(lang, String(audit.items.length) + " 条记录", String(audit.items.length) + " events"),
    href: restricted ? null : "/" + locale + "/admin/audit",
    actionLabel: restricted ? pickText(lang, "仅超级管理员可查看", "Super Admin only") : pickText(lang, "查看详情", "View details"),
  });

  return (
    <>
      <StatusBanner
        description={restricted
          ? pickText(lang, "当前为操作员权限边界，可继续处理审核与巡检；模板、归集、系统配置与完整审计仍受限制。", "Operator boundary is active. Reviews and supervision stay available, while templates, sweeps, system configuration, and full audit remain restricted.")
          : pickText(lang, "当前为超级管理员会话，可直接处理模板、系统配置、归集与完整审计。", "Super Admin session is active for templates, system configuration, sweeps, and full audit." )}
        title={profile.admin_access_granted ? pickText(lang, "管理员权限已生效", "Admin access granted") : pickText(lang, "管理员权限未生效", "Admin access missing")}
        tone={profile.admin_access_granted ? "success" : "warning"}
      />
      <AppShellSection
        description={pickText(lang, "后台首页只保留当前最关键的处理面板与状态看板，进入后可以直接点击处理。", "The homepage keeps only the most important control panels and status boards so operators can act immediately.")}
        eyebrow={pickText(lang, "管理总览", "Control Overview")}
        title={pickText(lang, "运营总览", "Operations Overview")}
      >
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-5 gap-6 mb-6">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "待处理充值", "Pending Deposits")}</CardTitle>
              <CardDescription>{pickText(lang, "需要人工复核", "Manual review queue")}</CardDescription>
            </CardHeader>
            <CardBody>{openDeposits}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "会员风险", "Membership Risk")}</CardTitle>
              <CardDescription>{pickText(lang, "宽限、冻结、撤销", "Grace, frozen, revoked")}</CardDescription>
            </CardHeader>
            <CardBody>{membershipRisk}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "异常暂停", "Runtime Incidents")}</CardTitle>
              <CardDescription>{pickText(lang, "策略运行阻塞", "Strategies blocked")}</CardDescription>
            </CardHeader>
            <CardBody>{runtimeIncidents}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "地址池压力", "Pool Pressure")}</CardTitle>
              <CardDescription>{pickText(lang, String(enabledAddresses.length) + " / " + String(pools.addresses.length) + " 可用地址", String(enabledAddresses.length) + " of " + String(pools.addresses.length) + " enabled")}</CardDescription>
            </CardHeader>
            <CardBody>{pressuredChains.length}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "审计事件", "Audit Events")}</CardTitle>
              <CardDescription>{restricted ? pickText(lang, "摘要模式", "Summary only") : pickText(lang, "完整可见", "Full visibility")}</CardDescription>
            </CardHeader>
            <CardBody>{auditEvents.length}</CardBody>
          </Card>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {panels.map((panel) => {
            const disabled = !panel.href;
            return (
              <Card key={panel.id}>
                <CardHeader>
                  <CardTitle>{panel.title}</CardTitle>
                  <CardDescription>{panel.description}</CardDescription>
                </CardHeader>
                <CardBody className="flex flex-col gap-4">
                  <div className="text-2xl font-semibold text-foreground">{panel.value}</div>
                  {panel.href ? (
                    <Link className={actionButtonClass(false)} href={panel.href}>
                      {panel.actionLabel}
                    </Link>
                  ) : (
                    <span className={actionButtonClass(disabled)}>{panel.actionLabel}</span>
                  )}
                </CardBody>
              </Card>
            );
          })}
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-6">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "当前权限", "Current permissions")}</CardTitle>
              <CardDescription>{pickText(lang, "先确认你的会话边界，再决定是否处理模板或系统变更。", "Confirm the session boundary before making template or system changes.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "权限角色：" + roleBoundaryLabel(lang, restricted), "Role boundary: " + roleBoundaryLabel(lang, restricted))}</li>
                <li>{restricted ? pickText(lang, "完整审计：仅超级管理员可见", "Full audit: Super Admin only") : pickText(lang, "完整审计：当前会话可见", "Full audit: available in this session")}</li>
                <li>{profile.admin_permissions?.can_manage_templates ? pickText(lang, "模板治理：当前会话可编辑", "Templates: editable in this session") : pickText(lang, "模板治理：当前会话只读", "Templates: read only in this session")}</li>
                <li>{pressuredChains.length > 0 ? pickText(lang, "地址池关注：" + pressuredChains.join("、") + " 需要补池", "Address pools to watch: " + pressuredChains.join(", ") + " need replenishment") : pickText(lang, "地址池关注：暂无异常", "Address pools to watch: no active issues")}</li>
              </ul>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "处理建议", "Suggested actions")}</CardTitle>
              <CardDescription>{pickText(lang, "按优先级给出下一步，不再展示生硬的入口字符串。", "Next steps are shown by priority instead of exposing raw route strings.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{openDeposits > 0 ? pickText(lang, "先处理待复核充值，避免会员订单持续堆积。", "Review pending deposits first so membership orders do not keep piling up.") : pickText(lang, "充值队列稳定，可以继续查看会员或策略状态。", "The deposit queue is stable. You can continue with memberships or strategy status.")}</li>
                <li>{membershipRisk > 0 ? pickText(lang, "存在宽限或冻结账号，建议同步检查支付与链上入账。", "There are grace or frozen accounts. Cross-check payments and on-chain credits next.") : pickText(lang, "会员生命周期稳定，可把重点放到策略运行与地址池。", "Membership lifecycle is stable, so focus can move to strategies and address pools.")}</li>
                <li>{runtimeIncidents > 0 ? pickText(lang, "异常暂停策略需要优先查看预检与事件流。", "Error-paused strategies should be reviewed through pre-flight and runtime events first.") : pickText(lang, "策略运行稳定，可把检查重点放在模板和地址池。", "Strategies are stable. Templates and address pools are the next review target.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "最近审计事件", "Recent Audit Events")}</CardTitle>
          <CardDescription>{pickText(lang, "保留时间、动作和目标，方便快速回溯最近管理员操作。", "Timestamp, action, and target remain visible for quick review of recent admin activity.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
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
              emptyMessage={restricted ? pickText(lang, "当前席位只能查看审计摘要，请切换到超级管理员会话查看明细。", "Summary only in this session. Use a Super Admin session for full audit detail.") : pickText(lang, "暂无审计事件。", "No audit events yet.")}
            />
          </div>
        </CardBody>
      </Card>
    </>
  );
}

