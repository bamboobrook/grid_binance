import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, Field, FormStack, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  searchParams?: Promise<{ error?: string | string[]; notice?: string | string[] }>;
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

export default async function BillingPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const notice = firstValue(params.notice);
  const error = firstValue(params.error);
  const overview = await fetchBillingOverview();
  const plans = overview?.plans ?? [];
  const orders = overview?.orders ?? [];
  const membership = overview?.membership ?? null;

  return (
    <>
      <StatusBanner
        description="Membership enters a 48-hour grace period after expiry. Existing strategies may continue only during that window."
        title="Grace-period reminder enabled"
        tone="warning"
      />
      {notice ? <StatusBanner description={notice} title="Awaiting exact transfer" tone="warning" /> : null}
      {error ? <StatusBanner description={error} title="Billing request failed" tone="warning" /> : null}
      <AppShellSection
        description="Create renewal orders with visible exact-amount warnings, plan pricing, and membership timing."
        eyebrow="Membership billing"
        title="Billing Center"
      >
        <div className="content-grid content-grid--metrics">
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
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Create payment order</CardTitle>
            <CardDescription>Renewal timing stays visible before the user sends funds on-chain.</CardDescription>
          </CardHeader>
          <CardBody>
            <p>Next renewal: {membership?.active_until?.slice(0, 10) ?? "Unavailable"}</p>
            <FormStack action="/api/user/billing" method="post">
              <Field label="Plan">
                <Select defaultValue="monthly" name="plan">
                  <option value="monthly">Monthly</option>
                  <option value="quarterly">Quarterly</option>
                  <option value="yearly">Yearly</option>
                </Select>
              </Field>
              <Field label="Chain">
                <Select defaultValue="bsc" name="chain">
                  <option value="ethereum">Ethereum</option>
                  <option value="bsc">BSC</option>
                  <option value="solana">Solana</option>
                </Select>
              </Field>
              <Field label="Token">
                <Select defaultValue="usdt" name="token">
                  <option value="usdt">USDT</option>
                  <option value="usdc">USDC</option>
                </Select>
              </Field>
              <Button type="submit">Create payment order</Button>
            </FormStack>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Membership timing</CardTitle>
            <CardDescription>Pricing changes apply to the following billing cycle, not the current entitlement.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Membership status: {membership?.status ?? "Unknown"}</li>
              <li>Renewal stacking: Allowed</li>
              <li>Grace period ends: {membership?.grace_until?.slice(0, 10) ?? "Unavailable"}</li>
              <li><Link href="/app/strategies">Strategy Workspace</Link></li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Payment orders</CardTitle>
            <CardDescription>Exact chain, token, and amount are all required for automatic confirmation.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: "Order" },
                { key: "chainToken", label: "Chain / token" },
                { key: "details", label: "Assignment details" },
                { key: "amount", label: "Amount", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={orders.map((row) => ({
                id: String(row.order_id),
                order: `ORD-${String(row.order_id).padStart(4, "0")}`,
                chainToken: `${row.chain} / ${row.asset}`,
                details: row.address
                  ? `Assigned address: ${row.address} | Address lock expires: ${row.expires_at?.slice(0, 19).replace("T", " ") ?? "pending"}`
                  : `Queue position: ${row.queue_position ?? "pending"} | Assigned address pending`,
                amount: row.amount,
                state: <Chip tone={row.status === "matched" || row.status === "completed" ? "success" : "warning"}>{row.status}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description="Payment amount must match exactly. Overpayment, underpayment, or wrong token will require manual review before membership can be extended."
          title="Payment amount must match exactly"
          tone="danger"
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
  const response = await fetch(`${authApiBaseUrl()}/billing/overview`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
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
