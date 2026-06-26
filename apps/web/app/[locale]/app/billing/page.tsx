import { cookies } from "next/headers";
import { CalendarClock, CreditCard, ShieldCheck, WalletCards } from "lucide-react";
import type { ReactNode } from "react";

import { MembershipOrderForm } from "@/components/billing/membership-order-form";
import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
  const previewMode = process.env.NEXT_PUBLIC_UI_PREVIEW === "1";
  const billingOverview = overview ?? (previewMode ? previewBillingOverview() : null);
  const plans = billingOverview?.plans ?? [];
  const orders = billingOverview?.orders ?? [];
  const membership = billingOverview?.membership ?? null;
  const activeUntil = membership?.active_until?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable");
  const graceUntil = membership?.grace_until?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable");
  const highlightedPlan = plans[0] ?? null;
  const overviewCards = [
    {
      icon: <ShieldCheck className="h-4 w-4" />,
      label: pickText(lang, "会员状态", "Membership status"),
      value: describeMembershipStatus(lang, membership?.status),
      detail: membership?.active_until
        ? pickText(lang, `到期 ${activeUntil}`, `Expires ${activeUntil}`)
        : pickText(lang, "创建订单后会自动更新", "Updates after payment"),
    },
    {
      icon: <CalendarClock className="h-4 w-4" />,
      label: pickText(lang, "会员到期", "Expires"),
      value: activeUntil,
      detail: pickText(lang, `宽限期 ${graceUntil}`, `Grace period ${graceUntil}`),
    },
    {
      icon: <CreditCard className="h-4 w-4" />,
      label: pickText(lang, "推荐套餐", "Plan"),
      value: highlightedPlan ? labelForPlan(lang, highlightedPlan.code, highlightedPlan.name) : pickText(lang, "暂无套餐", "No plan"),
      detail: highlightedPlan ? firstUsdAmount(highlightedPlan) : pickText(lang, "请联系管理员", "Contact support"),
    },
    {
      icon: <WalletCards className="h-4 w-4" />,
      label: pickText(lang, "支付订单", "Payment orders"),
      value: pickText(lang, `${orders.length} 笔`, `${orders.length} orders`),
      detail: orders.length > 0 ? pickText(lang, "查看付款状态", "Check payment status") : pickText(lang, "还没有订单", "No orders yet"),
    },
  ];
  const planOptions = plans.length > 0 ? plans : [];

  return (
    <>
      {notice ? <StatusBanner description={notice} title={pickText(lang, "等待精确到账", "Awaiting exact transfer")}  tone="info" lang={lang} /> : null}
      {error ? <StatusBanner description={error} title={pickText(lang, "会员请求失败", "Membership request failed")}  tone="info" lang={lang} /> : null}
      <AppShellSection
        eyebrow={pickText(lang, "会员服务", "Membership service")}
        title={pickText(lang, "会员中心", "Membership Center")}
      >
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {overviewCards.map((card) => (
            <OverviewCard
              detail={card.detail}
              icon={card.icon}
              key={card.label}
              label={card.label}
              value={card.value}
            />
          ))}
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "创建支付订单", "Create payment order")}</CardTitle>
              <CardDescription>{pickText(lang, "选择套餐、链路和币种后生成订单。付款时按订单显示金额转账。", "Choose a plan, chain, and token, then pay the exact amount shown on the order.")}</CardDescription>
            </CardHeader>
            <CardBody className="space-y-4">
              <div className="rounded-md border-2 border-primary bg-primary/10 px-4 py-3">
                <p className="text-sm font-black text-foreground">{pickText(lang, "重要：支付金额必须精确匹配", "Important: payment amount must match exactly")}</p>
                <p className="mt-1 text-xs leading-relaxed text-foreground/80">
                  {pickText(lang, "链路、币种和金额都要和订单一致。多转、少转或转错币种会进入人工复核。", "Chain, token, and amount must match the order. Wrong or mismatched payments require manual review.")}
                </p>
              </div>
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

          <div className="flex flex-col gap-4 self-start">
            <Card>
              <CardHeader>
                <CardTitle>{pickText(lang, "会员套餐", "Membership plans")}</CardTitle>
              </CardHeader>
              <CardBody>
                {planOptions.length > 0 ? (
                  <div className="grid gap-2">
                    {planOptions.map((plan) => (
                      <div className="rounded-sm border border-border bg-background p-3" key={plan.code}>
                        <div className="flex items-center justify-between gap-3">
                          <p className="text-sm font-bold">{labelForPlan(lang, plan.code, plan.name)}</p>
                          <strong className="text-sm font-black">{firstUsdAmount(plan)}</strong>
                        </div>
                        <p className="mt-1 text-xs text-muted-foreground">{describePlanPaymentOptions(lang, plan)}</p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">{pickText(lang, "暂无可用套餐", "No plans available")}</p>
                )}
              </CardBody>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>{pickText(lang, "付款规则", "Payment rules")}</CardTitle>
              </CardHeader>
              <CardBody>
                <ul className="space-y-3 text-sm">
                  <li className="flex gap-3">
                    <span className="mt-1 h-2 w-2 shrink-0 rounded-full bg-primary" />
                    <span>{pickText(lang, "金额必须和订单完全一致。", "Amount must match the order exactly.")}</span>
                  </li>
                  <li className="flex gap-3">
                    <span className="mt-1 h-2 w-2 shrink-0 rounded-full bg-primary" />
                    <span>{pickText(lang, "链路和稳定币不要选错。", "Use the selected chain and stablecoin.")}</span>
                  </li>
                  <li className="flex gap-3">
                    <span className="mt-1 h-2 w-2 shrink-0 rounded-full bg-primary" />
                    <span>{pickText(lang, "付款后在下方订单里查看状态。", "After payment, check the order status below.")}</span>
                  </li>
                </ul>
              </CardBody>
            </Card>
          </div>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "支付订单", "Payment orders")}</CardTitle>
            <CardDescription>{pickText(lang, "创建订单后，这里会显示金额、地址和确认状态。", "After creating an order, its amount, address, and status appear here.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: pickText(lang, "订单", "Order") },
                { key: "chainToken", label: pickText(lang, "链路 / 币种", "Chain / token") },
                { key: "details", label: pickText(lang, "付款信息", "Payment info") },
                { key: "amount", label: pickText(lang, "金额", "Amount"), align: "right" },
                { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
              ]}
              emptyMessage={pickText(lang, "暂无支付订单。创建续费订单后会显示在这里。", "No payment orders yet. Create a renewal order to see it here.")}
              rows={orders.map((row) => ({
                id: String(row.order_id),
                order: "ORD-" + String(row.order_id).padStart(4, "0"),
                chainToken: row.chain + " / " + row.asset,
                details: row.address
                  ? (
                    <div className="max-w-md">
                      <p className="break-all font-mono text-xs">{row.address}</p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {pickText(lang, "锁定到期", "Address lock expires")}: {formatTaipeiDateTime(row.expires_at, lang, { fallback: pickText(lang, "处理中", "pending") })}
                      </p>
                    </div>
                  )
                  : (
                    <div>
                      <p>{pickText(lang, "等待分配地址", "Assigned address pending")}</p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {pickText(lang, "排队序号", "Queue position")}: {String(row.queue_position ?? pickText(lang, "处理中", "pending"))}
                      </p>
                    </div>
                  ),
                amount: `${row.amount} ${row.asset}`,
                state: formatOrderStatus(lang, row.status),
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}

function OverviewCard({
  detail,
  icon,
  label,
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-md border border-border bg-card p-4">
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <p className="text-xs font-bold uppercase">{label}</p>
      </div>
      <p className="mt-2 text-sm font-bold text-foreground">{value}</p>
      <p className="mt-1 text-xs text-muted-foreground">{detail}</p>
    </div>
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

function previewBillingOverview(): BillingOverview {
  return {
    membership: {
      active_until: "2026-08-18T15:59:59Z",
      grace_until: "2026-08-20T15:59:59Z",
      status: "Active",
    },
    plans: [
      {
        code: "monthly",
        name: "Monthly",
        prices: [
          { amount: "19.90", asset: "USDT", chain: "TRC20" },
          { amount: "19.90", asset: "USDT", chain: "BEP20" },
        ],
      },
      {
        code: "quarterly",
        name: "Quarterly",
        prices: [
          { amount: "49.90", asset: "USDT", chain: "TRC20" },
          { amount: "49.90", asset: "USDC", chain: "BEP20" },
        ],
      },
      {
        code: "yearly",
        name: "Yearly",
        prices: [
          { amount: "169.00", asset: "USDT", chain: "TRC20" },
          { amount: "169.00", asset: "USDC", chain: "BEP20" },
        ],
      },
    ],
    orders: [
      {
        address: "TQ8b2mN9pVa7V5U6tBmZpM6AGp48QbHhQ2",
        amount: "49.90",
        asset: "USDT",
        chain: "TRC20",
        expires_at: "2026-06-16T14:30:00Z",
        order_id: 1286,
        queue_position: null,
        status: "pending",
      },
      {
        address: "0x7fd4a557b7f9b2d2c8e2402c3ef9d4c2b7852d11",
        amount: "19.90",
        asset: "USDT",
        chain: "BEP20",
        expires_at: "2026-05-21T10:20:00Z",
        order_id: 1208,
        queue_position: null,
        status: "confirmed",
      },
    ],
  };
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

function describePlanPaymentOptions(lang: UiLanguage, plan: BillingPlan) {
  const chains = Array.from(new Set(plan.prices.map((price) => price.chain))).join(" / ");
  const assets = Array.from(new Set(plan.prices.map((price) => price.asset))).join(" / ");
  if (!chains && !assets) {
    return pickText(lang, "暂无支付方式", "No payment options");
  }
  return pickText(lang, `${chains} · ${assets}`, `${chains} · ${assets}`);
}

function formatOrderStatus(lang: UiLanguage, status: string) {
  switch (status.trim().toLowerCase()) {
    case "pending":
      return pickText(lang, "待支付", "Pending");
    case "waiting_address":
      return pickText(lang, "等待地址", "Waiting for address");
    case "paid":
    case "confirmed":
      return pickText(lang, "已确认", "Confirmed");
    case "expired":
      return pickText(lang, "已过期", "Expired");
    case "manual_review":
      return pickText(lang, "人工复核", "Manual review");
    default:
      return status || pickText(lang, "处理中", "Processing");
  }
}
