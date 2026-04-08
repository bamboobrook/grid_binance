import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { DialogFrame } from "@/components/ui/dialog";
import { Button, Field, FormStack, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute } from "@/lib/ui/preferences";

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

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function BillingPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
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
  const selectedPlan = plans.find((plan) => plan.code === requestedPlan) ?? plans[0] ?? null;
  const selectedPrice = selectedPlan?.prices.find((price) => {
    const chainMatches = !requestedChain || price.chain === requestedChain;
    const tokenMatches = !requestedToken || price.asset === requestedToken;
    return chainMatches && tokenMatches;
  }) ?? selectedPlan?.prices[0] ?? null;
  const chainOptions = uniqueValues((selectedPlan?.prices ?? []).map((price) => price.chain));
  const tokenOptions = uniqueValues((selectedPlan?.prices ?? []).map((price) => price.asset));

  return (
    <>
      <StatusBanner
        description={pickText(lang, "会员到期后会进入 48 小时宽限期；仅已运行策略可在窗口期内继续。", "Membership enters a 48-hour grace period after expiry, and only already-running strategies may continue during that window.")}
        title={pickText(lang, "宽限期提醒已启用", "Grace-period reminder enabled")}
      />
      {notice ? <StatusBanner description={notice} title={pickText(lang, "等待精确转账", "Awaiting exact transfer")} /> : null}
      {error ? <StatusBanner description={error} title={pickText(lang, "计费请求失败", "Billing request failed")} tone="danger" /> : null}
      <AppShellSection
        description={pickText(lang, "这里创建续费订单、查看精确金额要求，并确认会员时效。", "Create renewal orders, review exact transfer requirements, and confirm membership timing here.")}
        eyebrow={pickText(lang, "会员计费", "Membership billing")}
        title={pickText(lang, "计费中心", "Billing Center")}
      >
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 mb-6">
          {plans.map((item) => (
            <Card key={item.code}>
              <CardHeader>
                <CardTitle>{item.name}</CardTitle>
                <CardDescription>{item.prices.map((price) => `${price.chain} ${price.asset} ${price.amount}`).join(" | ")}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "创建支付订单", "Create payment order")}</CardTitle>
            <CardDescription>{pickText(lang, "转账前先确认精确金额、链路和代币。", "Confirm the exact amount, chain, and token before sending funds on-chain.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <p>{pickText(lang, "下次到期", "Next renewal")}: {membership?.active_until?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable")}</p>
            <p>{pickText(lang, "当前选择", "Selected price")}: {selectedPrice ? `${selectedPrice.chain} ${selectedPrice.asset} ${selectedPrice.amount}` : pickText(lang, "暂无可用价格", "No price available")}</p>
            <FormStack action="/api/user/billing" method="post">
              <Field label={pickText(lang, "套餐", "Plan")}>
                <Select defaultValue={selectedPlan?.code ?? ""} name="plan">
                  {plans.length === 0 ? <option value="">{pickText(lang, "暂无套餐", "No plans available")}</option> : null}
                  {plans.map((plan) => (
                    <option key={plan.code} value={plan.code}>{plan.name}</option>
                  ))}
                </Select>
              </Field>
              <Field label={pickText(lang, "链", "Chain")}>
                <Select defaultValue={selectedPrice?.chain ?? ""} name="chain">
                  {chainOptions.length === 0 ? <option value="">{pickText(lang, "暂无链路", "No chain available")}</option> : null}
                  {chainOptions.map((chain) => (
                    <option key={chain} value={chain}>{chain}</option>
                  ))}
                </Select>
              </Field>
              <Field label={pickText(lang, "代币", "Token")}>
                <Select defaultValue={selectedPrice?.asset ?? ""} name="token">
                  {tokenOptions.length === 0 ? <option value="">{pickText(lang, "暂无代币", "No token available")}</option> : null}
                  {tokenOptions.map((token) => (
                    <option key={token} value={token}>{token}</option>
                  ))}
                </Select>
              </Field>
              <Button type="submit">{pickText(lang, "创建支付订单", "Create payment order")}</Button>
            </FormStack>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "会员时效", "Membership timing")}</CardTitle>
            <CardDescription>{pickText(lang, "价格变更只影响下一次续费，不回溯当前权益。", "Price changes affect the next renewal only and do not backdate the current entitlement.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "会员状态", "Membership status")}: {membership?.status ?? pickText(lang, "未知", "Unknown")}</li>
              <li>{pickText(lang, "续费叠加", "Renewal stacking")}: {pickText(lang, "允许", "Allowed")}</li>
              <li>{pickText(lang, "宽限期结束", "Grace period ends")}: {membership?.grace_until?.slice(0, 10) ?? pickText(lang, "暂无", "Unavailable")}</li>
              <li><Link href={`/${locale}/app/strategies`}>{pickText(lang, "前往策略工作区", "Open strategy workspace")}</Link></li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "支付订单", "Payment orders")}</CardTitle>
            <CardDescription>{pickText(lang, "自动确认要求链、币种和金额完全一致。", "Automatic confirmation requires an exact chain, token, and amount match.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">
                <DataTable
              columns={[
                { key: "order", label: pickText(lang, "订单", "Order") },
                { key: "chainToken", label: pickText(lang, "链 / 代币", "Chain / token") },
                { key: "details", label: pickText(lang, "分配详情", "Assignment details") },
                { key: "amount", label: pickText(lang, "金额", "Amount"), align: "right" },
                { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
              ]}
              rows={orders.map((row) => ({
                id: String(row.order_id),
                order: `ORD-${String(row.order_id).padStart(4, "0")}`,
                chainToken: `${row.chain} / ${row.asset}`,
                details: row.address
                  ? pickText(lang, `分配地址：${row.address} | 锁定到期：${row.expires_at?.slice(0, 19).replace("T", " ") ?? "待定"}`, `Assigned address: ${row.address} | Lock expires: ${row.expires_at?.slice(0, 19).replace("T", " ") ?? "pending"}`)
                  : pickText(lang, `排队位置：${String(row.queue_position ?? "待定")} | 地址待分配`, `Queue position: ${String(row.queue_position ?? "pending")} | Address pending`),
                amount: row.amount,
                state: <Chip tone={row.status === "matched" || row.status === "completed" ? "success" : "warning"}>{row.status}</Chip>,
              }))}
            />
              </div>
          </CardBody>
        </Card>
        <DialogFrame
          description={pickText(lang, "支付金额必须完全一致；多付、少付或转错币种都会进入人工审核。", "The payment amount must match exactly. Overpayment, underpayment, or the wrong token will require manual review.")}
          title={pickText(lang, "金额必须精确一致", "Payment amount must match exactly")}
        />
      </div>
    </>
  );
}

function uniqueValues(values: string[]) {
  return Array.from(new Set(values));
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
