import Link from "next/link";
import { cookies } from "next/headers";

import { MembershipOrderForm } from "@/components/billing/membership-order-form";
import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DialogFrame } from "@/components/ui/dialog";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { describeMembershipStatus } from "@/lib/ui/domain-copy";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ error?: string | string[]; notice?: string | string[]; plan?: string | string[]; chain?: string | string[]; token?: string | string[] }>;
};

type BillingOverview = {
  membership: {
    grace_until?: string | null;
    status: string;
    active_until?: string | null;
  };
  orders: Array<{
    address: string | null;
    amount: string;
    asset: string;
    chain: string;
    order_id: number;
    queue_position: number | null;
    status: string;
    expires_at?: string | null;
  }>;
  plans: Array<{
    code: string;
    name: string;
    prices: Array<{ amount: string; asset: string; chain: string }>;
  }>;
};

type BillingPlan = BillingOverview["plans"][number];


function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function BillingPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const query = (await searchParams) ?? {};
  const notice = firstValue(query.notice);
  const error = firstValue(query.error);
  const requestedPlan = firstValue(query.plan) ?? "";
  const requestedChain = firstValue(query.chain) ?? "";
  const requestedToken = firstValue(query.token) ?? "";
  const overview = await fetchBillingOverview();
  const plans = overview?.plans ?? [];
  const orders = overview?.orders ?? [];
  const membership = overview?.membership ?? null;

  return (
    <>
      <StatusBanner
        description={pickText(lang, "会员到期后会进入48小时宽限期，宽限期内已运行策略可继续运行。", "Membership enters a 48-hour grace period after expiry. Existing strategies may continue only during that window.")}
        title={pickText(lang, "宽限期规则已启用", "Grace-period reminder enabled")}
      />
      {notice ? <StatusBanner description={notice} title={pickText(lang, "等待精确到账", "Awaiting exact transfer")} /> : null}
      {error ? <StatusBanner description={error} title={pickText(lang, "会员请求失败", "Membership request failed")} /> : null}
      <AppShellSection
        description={pickText(lang, "在这里创建续费订单、确认精确金额，并查看会员时间线。", "Create renewal orders, confirm the exact amount, and review membership timing here.")}
        eyebrow={pickText(lang, "会员服务", "Membership service")}
        title={pickText(lang, "会员中心", "Membership Center")}
      >
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "会员套餐", "Membership plans")}</CardTitle>
            <CardDescription>{pickText(lang, "这里只保留简洁套餐说明；具体下单金额会根据你下面选择的链路与稳定币实时联动。", "This section keeps plan pricing simple. The exact order amount below updates live with the selected chain and stablecoin.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {plans.map((plan) => (
                <li key={plan.code}>{describePlanSummary(lang, plan)}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </AppShellSection>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "创建支付订单", "Create payment order")}</CardTitle>
            <CardDescription>{pickText(lang, "链路、币种和金额都必须完全一致，系统才会自动确认。", "Chain, token, and amount must match exactly before the system can confirm automatically.")}</CardDescription>
          </CardHeader>
          <CardBody className="space-y-4">
            <MembershipOrderForm
              activeUntil={membership?.active_until}
              initialChain={requestedChain}
              initialPlanCode={requestedPlan}
              initialToken={requestedToken}
              lang={lang}
              plans={plans}
            />
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "会员时间线", "Membership timing")}</CardTitle>
            <CardDescription>{pickText(lang, "价格调整只影响后续续费，不影响当前已生效会员。", "Pricing changes apply to later renewals, not the currently active entitlement.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "会员状态", "Membership status")}: {describeMembershipStatus(lang, membership?.status)}</li>
              <li>{pickText(lang, "续费叠加", "Renewal stacking")}: {pickText(lang, "允许", "Allowed")}</li>
              <li>{pickText(lang, "宽限期截止", "Grace period ends")}: {membership?.grace_until?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable")}</li>
              <li><Link href={`/${locale}/app/strategies`}>{pickText(lang, "前往策略工作台", "Open strategy workspace")}</Link></li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "支付订单", "Payment orders")}</CardTitle>
            <CardDescription>{pickText(lang, "地址分配、锁定时间和排队状态都会在这里显示。", "Address assignment, lock timing, and queue state stay visible here.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: pickText(lang, "订单", "Order") },
                { key: "chainToken", label: pickText(lang, "链路 / 币种", "Chain / token") },
                { key: "details", label: pickText(lang, "分配详情", "Assignment details") },
                { key: "amount", label: pickText(lang, "金额", "Amount"), align: "right" },
                { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
              ]}
              rows={orders.map((row) => ({
                id: String(row.order_id),
                order: "ORD-" + String(row.order_id).padStart(4, "0"),
                chainToken: row.chain + " / " + row.asset,
                details: row.address
                  ? pickText(lang, "已分配地址：", "Assigned address: ") + row.address + " | " + pickText(lang, "锁定到期：", "Address lock expires: ") + formatTaipeiDateTime(row.expires_at, lang, { fallback: pickText(lang, "处理中", "pending") })
                  : pickText(lang, "排队序号：", "Queue position: ") + String(row.queue_position ?? pickText(lang, "处理中", "pending")) + " | " + pickText(lang, "等待分配地址", "Assigned address pending"),
                amount: row.amount,
                state: row.status,
              }))}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description={pickText(lang, "支付金额必须完全一致。多转、少转或转错币种都会进入人工复核。", "Payment amount must match exactly. Overpayment, underpayment, or wrong token will require manual review before membership can be extended.")}
          lang={lang}
          title={pickText(lang, "支付金额必须精确匹配", "Payment amount must match exactly")}
        />
      </div>
    </>
  );
}

async function fetchBillingOverview(): Promise<BillingOverview | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const response = await fetch(authApiBaseUrl() + "/billing/overview", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as BillingOverview;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function describePlanSummary(lang: UiLanguage, plan: BillingPlan) {
  return `${labelForPlan(lang, plan.code, plan.name)} ${firstUsdAmount(plan)}`;
}

function labelForPlan(lang: UiLanguage, code: string, fallback: string) {
  switch (code.trim().toLowerCase()) {
    case "monthly":
      return pickText(lang, "按月支付", "Pay monthly");
    case "quarterly":
      return pickText(lang, "按季度支付", "Pay quarterly");
    case "yearly":
      return pickText(lang, "按年支付", "Pay yearly");
    default:
      return fallback;
  }
}

function firstUsdAmount(plan: BillingPlan) {
  return `${plan.prices[0]?.amount ?? "0"} USD`;
}
