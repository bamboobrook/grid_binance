import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable, type DataTableColumn } from "@/components/ui/table";
import {
  getAdminMembershipPlansData,
  getAdminMembershipsData,
  getCurrentAdminProfile,
} from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

const SUPPORTED_CHAINS = ["ETH", "BSC", "SOL"] as const;
const SUPPORTED_ASSETS = ["USDT", "USDC"] as const;
const SUPPORTED_PRICE_MATRIX = SUPPORTED_CHAINS.flatMap((chain) =>
  SUPPORTED_ASSETS.map((asset) => ({ chain, asset, fieldName: "price:" + chain + ":" + asset })),
);

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ action?: string; plan?: string; planError?: string; planSaved?: string; target?: string }>;
};

function membershipStatusLabel(lang: UiLanguage, status: string) {
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
      return status;
  }
}

function actionLabel(lang: UiLanguage, action: string) {
  switch (action) {
    case "open":
      return pickText(lang, "会员已开通", "Membership opened");
    case "extend":
      return pickText(lang, "会员已延长", "Membership extended");
    case "freeze":
      return pickText(lang, "会员已冻结", "Membership frozen");
    case "unfreeze":
      return pickText(lang, "会员已解冻", "Membership unfrozen");
    case "revoke":
      return pickText(lang, "会员已撤销", "Membership revoked");
    default:
      return action;
  }
}

export default async function AdminMembershipsPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const [cookieStore, profile, memberships, plans] = await Promise.all([
    cookies(),
    getCurrentAdminProfile(),
    getAdminMembershipsData(),
    getAdminMembershipPlansData(),
  ]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const targetEmail = typeof query.target === "string" ? query.target : "";
  const lastAction = typeof query.action === "string" ? query.action : "";
  const selectedPlanCode = typeof query.plan === "string" ? query.plan : "";
  const planSaved = typeof query.planSaved === "string" ? query.planSaved : "";
  const planError = typeof query.planError === "string" ? query.planError : "";
  const updatedMembership = memberships.items.find((item) => item.email === targetEmail) ?? null;
  const canManage = profile.admin_permissions?.can_manage_memberships ?? false;
  const canManagePlans = profile.admin_permissions?.can_manage_plans ?? false;
  const defaultPlan = plans.plans.find((plan) => plan.code === selectedPlanCode) ?? plans.plans[0] ?? null;
  const activePlanCount = plans.plans.filter((plan) => plan.is_active).length;
  const riskCount = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const priceFor = (chain: string, asset: string) => defaultPlan?.prices.find((price) => price.chain === chain && price.asset === asset)?.amount ?? "";
  const membershipColumns: DataTableColumn[] = [
    { key: "email", label: pickText(lang, "会员", "Member") },
    { key: "status", label: pickText(lang, "生命周期", "Lifecycle") },
    { key: "activeUntil", label: pickText(lang, "有效至", "Active Until") },
    { key: "graceUntil", label: pickText(lang, "宽限至", "Grace Until") },
  ];

  if (canManage) {
    membershipColumns.push({ key: "actions", label: pickText(lang, "值班动作", "Desk Actions") });
  }

  return (
    <>
      {updatedMembership && lastAction ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "目标账号：" + updatedMembership.email + "，当前状态：" + membershipStatusLabel(lang, updatedMembership.status) + "，最近动作：" + actionLabel(lang, lastAction), "Target " + updatedMembership.email + ". Status " + membershipStatusLabel(lang, updatedMembership.status) + ". Last action " + actionLabel(lang, lastAction) + ".")}
          title={pickText(lang, "会员变更已记录", "Membership Change Recorded")}
         
        />
      ) : null}
      {planSaved ? <StatusBanner description={pickText(lang, "已保存计划：" + planSaved, "Saved plan: " + planSaved)} title={pickText(lang, "价格矩阵已保存", "Price Matrix Saved")}  tone="info" lang={lang} /> : null}
      {planError ? <StatusBanner description={planError} title={pickText(lang, "价格矩阵未保存", "Price Matrix Not Saved")}  tone="info" lang={lang} /> : null}
      <AppShellSection
        description={pickText(lang, "值班席位同时处理会员生命周期与价格矩阵；价格编辑始终绑定当前选中的计划。", "The desk handles lifecycle and pricing, and the editor always binds to the currently selected plan.")}
        eyebrow={pickText(lang, "会员生命周期", "Membership Lifecycle")}
        title={pickText(lang, "会员运营", "Membership Operations")}
      >
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 mb-6">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "风险会员", "Membership Risk")}</CardTitle>
              <CardDescription>{pickText(lang, "宽限、冻结、撤销", "Grace, Frozen, Revoked")}</CardDescription>
            </CardHeader>
            <CardBody>{riskCount}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "生效计划", "Active Plans")}</CardTitle>
              <CardDescription>{pickText(lang, "计划快照", "Plan Snapshot")}</CardDescription>
            </CardHeader>
            <CardBody>{activePlanCount}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "权限边界", "Permission Boundary")}</CardTitle>
              <CardDescription>{canManage ? pickText(lang, "可处理会员动作", "Membership actions enabled") : pickText(lang, "当前席位只读", "Read-only desk")}</CardDescription>
            </CardHeader>
            <CardBody>{canManagePlans ? pickText(lang, "价格矩阵可编辑", "Price Matrix Editable") : pickText(lang, "价格矩阵只读", "Price Matrix Read-only")}</CardBody>
          </Card>
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "价格矩阵", "Price Matrix")}</CardTitle>
              <CardDescription>{pickText(lang, "先选择一个计划，再编辑它的展示名称、时长和链上报价。", "Choose a plan first, then edit its name, duration, and on-chain prices.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {canManagePlans ? (
                <>
                  <FormStack action={`/${locale}/admin/memberships`} className="mb-4" method="get">
                    <Field label={pickText(lang, "当前计划", "Selected Plan")}>
                      <select className="flex h-9 w-full rounded-sm border border-border bg-input px-3 py-1 text-sm shadow-sm" defaultValue={defaultPlan?.code ?? ""} name="plan">
                        {plans.plans.map((plan) => (
                          <option key={plan.code} value={plan.code}>{plan.code}</option>
                        ))}
                      </select>
                    </Field>
                    <Button type="submit">{pickText(lang, "加载计划", "Load Plan")}</Button>
                  </FormStack>
                  <FormStack action="/api/admin/memberships" method="post">
                    <input name="intent" type="hidden" value="save-plan" />
                    <Field label={pickText(lang, "计划代码", "Plan Code")}>
                      <Input defaultValue={defaultPlan?.code ?? ""} name="code" />
                    </Field>
                    <Field label={pickText(lang, "展示名称", "Display Name")}>
                      <Input defaultValue={defaultPlan?.name ?? ""} name="name" />
                    </Field>
                    <Field label={pickText(lang, "计划天数", "Duration Days")}>
                      <Input defaultValue={String(defaultPlan?.duration_days ?? 30)} inputMode="numeric" name="durationDays" />
                    </Field>
                    {SUPPORTED_PRICE_MATRIX.map(({ chain, asset, fieldName }) => (
                      <Field key={fieldName} label={pickText(lang, chain + " / " + asset + " 价格", chain + " / " + asset + " Price")}>
                        <Input defaultValue={priceFor(chain, asset)} name={fieldName} />
                      </Field>
                    ))}
                    <Button type="submit">{pickText(lang, "保存价格矩阵", "Save Price Matrix")}</Button>
                  </FormStack>
                </>
              ) : (
                <p>{pickText(lang, "需要 super_admin 才能改价格矩阵；当前席位只展示价格快照。", "A super_admin session is required to change the price matrix.")}</p>
              )}
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "会员开通", "Open Membership")}</CardTitle>
              <CardDescription>{pickText(lang, "用于补开、恢复或临时延长，不依赖预选行。", "Open, restore, or extend access without depending on a preselected row.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {canManage ? (
                <FormStack action="/api/admin/memberships" method="post">
                  <Field label={pickText(lang, "会员邮箱", "Member Email")}>
                    <Input name="email" placeholder="member@example.com" />
                  </Field>
                  <Field label={pickText(lang, "开通天数", "Duration Days")}>
                    <Input defaultValue="30" inputMode="numeric" name="durationDays" />
                  </Field>
                  <input name="action" type="hidden" value="open" />
                  <Button type="submit">{pickText(lang, "开通会员", "Open Membership")}</Button>
                </FormStack>
              ) : (
                <p>{pickText(lang, "当前席位只能查看生命周期，不可直接修改会员状态。", "This desk can review lifecycle state but cannot change it.")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "计划快照", "Plan Snapshot")}</CardTitle>
          <CardDescription>{pickText(lang, "展示全部计划的当前天数与链上报价，避免把当前能力夸大成“全计划都可直接编辑”。", "Shows all plans with duration and chain pricing so the desk does not overstate what is directly editable.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="overflow-x-auto min-w-full rounded-lg pb-4">
          <DataTable
            columns={[
              { key: "code", label: pickText(lang, "计划", "Plan") },
              { key: "duration", label: pickText(lang, "时长", "Duration") },
              { key: "active", label: pickText(lang, "生效", "Active") },
              { key: "prices", label: pickText(lang, "价格矩阵", "Price Matrix") },
            ]}
            rows={plans.plans.map((plan) => ({
              id: plan.code,
              active: plan.is_active ? pickText(lang, "已启用", "Active") : pickText(lang, "停用", "Inactive"),
              code: plan.code,
              duration: pickText(lang, String(plan.duration_days) + " 天", String(plan.duration_days) + " days"),
              prices: plan.prices.map((price) => price.chain + " " + price.asset + " " + price.amount).join(" | "),
            }))}
          />
          </div>
        </CardBody>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "会员主表", "Membership Desk Table")}</CardTitle>
          <CardDescription>{pickText(lang, "显式展示生命周期状态和可执行动作。", "Lifecycle status and available desk actions remain explicit.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="overflow-x-auto min-w-full rounded-lg pb-4">
          <DataTable
            columns={membershipColumns}
            rows={memberships.items.map((item) => ({
              id: item.email,
              activeUntil: item.active_until?.slice(0, 10) ?? "-",
              actions: canManage ? (
                <div className="min-w-[220px] space-y-3">
                  <div className="rounded-xl border border-border bg-background px-3 py-3">
                    <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      {pickText(lang, "延长会员", "Extend membership")}
                    </p>
                    <FormStack action="/api/admin/memberships" className="gap-2" method="post">
                      <input name="email" type="hidden" value={item.email} />
                      <Field label={pickText(lang, "延长天数", "Extend Days")}>
                        <Input defaultValue="15" inputMode="numeric" name="durationDays" />
                      </Field>
                      <input name="action" type="hidden" value="extend" />
                      <Button type="submit">{pickText(lang, "延长会员", "Extend Membership")}</Button>
                    </FormStack>
                  </div>
                  <div className="grid gap-2 md:grid-cols-2">
                    <FormStack action="/api/admin/memberships" method="post">
                      <input name="email" type="hidden" value={item.email} />
                      <input name="action" type="hidden" value={item.status === "Frozen" ? "unfreeze" : "freeze"} />
                      <Button type="submit">{item.status === "Frozen" ? pickText(lang, "解除冻结", "Unfreeze") : pickText(lang, "冻结会员", "Freeze")}</Button>
                    </FormStack>
                    <FormStack action="/api/admin/memberships" method="post">
                      <input name="email" type="hidden" value={item.email} />
                      <input name="action" type="hidden" value="revoke" />
                      <Button type="submit">{pickText(lang, "撤销会员", "Revoke")}</Button>
                    </FormStack>
                  </div>
                </div>
              ) : null,
              email: item.email,
              graceUntil: item.grace_until?.slice(0, 10) ?? "-",
              status: membershipStatusLabel(lang, item.status),
            }))}
          />
          </div>
        </CardBody>
      </Card>
    </>
  );
}
