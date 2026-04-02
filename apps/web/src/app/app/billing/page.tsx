import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, Field, FormStack, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { firstValue } from "../../../lib/auth";

type BillingPageProps = {
  searchParams?: Promise<{
    chain?: string | string[];
    create?: string | string[];
    plan?: string | string[];
    token?: string | string[];
  }>;
};

const planCatalog = {
  monthly: { label: "Monthly", amount: "20.00" },
  quarterly: { label: "Quarterly", amount: "54.00" },
  yearly: { label: "Yearly", amount: "180.00" },
} as const;

const chainLabels = {
  ethereum: "Ethereum",
  bsc: "BSC",
  solana: "Solana",
} as const;

const tokenLabels = {
  usdt: "USDT",
  usdc: "USDC",
} as const;

export default async function BillingPage({ searchParams }: BillingPageProps) {
  const params = (await searchParams) ?? {};
  const plan = (firstValue(params.plan) ?? "monthly") as keyof typeof planCatalog;
  const chain = (firstValue(params.chain) ?? "bsc") as keyof typeof chainLabels;
  const token = (firstValue(params.token) ?? "usdt") as keyof typeof tokenLabels;
  const create = firstValue(params.create) === "1";
  const selectedPlan = planCatalog[plan] ?? planCatalog.monthly;
  const selectedChain = chainLabels[chain] ?? chainLabels.bsc;
  const selectedToken = tokenLabels[token] ?? tokenLabels.usdt;
  const newOrder = create
    ? {
        id: "order-4201",
        order: "ORD-4201",
        chain: `${selectedChain} / ${selectedToken}`,
        amount: selectedPlan.amount,
        state: "Awaiting exact transfer",
      }
    : null;

  const existingOrders = [
    { id: "order-4138", order: "ORD-4138", chain: "Solana / USDC", amount: "60.00", state: "Confirmed" },
  ];
  const rows = newOrder ? [newOrder, ...existingOrders] : existingOrders;

  return (
    <>
      <StatusBanner
        description="Membership enters a 48-hour grace period after expiry. Existing strategies may continue only during that window."
        title="Grace-period reminder enabled"
        tone="warning"
      />
      <AppShellSection
        description="Create renewal orders with visible exact-amount warnings, plan pricing, and membership timing."
        eyebrow="Membership billing"
        title="Billing Center"
      >
        <div className="content-grid content-grid--metrics">
          {Object.values(planCatalog).map((item) => (
            <Card key={item.label}>
              <CardHeader>
                <CardTitle>{item.label}</CardTitle>
                <CardDescription>{item.amount} USD equivalent</CardDescription>
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
            <p>Next renewal: 2026-04-15</p>
            <FormStack action="/app/billing" method="get">
              <Field label="Plan">
                <Select defaultValue={plan} name="plan">
                  <option value="monthly">Monthly</option>
                  <option value="quarterly">Quarterly</option>
                  <option value="yearly">Yearly</option>
                </Select>
              </Field>
              <Field label="Chain">
                <Select defaultValue={chain} name="chain">
                  <option value="ethereum">Ethereum</option>
                  <option value="bsc">BSC</option>
                  <option value="solana">Solana</option>
                </Select>
              </Field>
              <Field label="Token">
                <Select defaultValue={token} name="token">
                  <option value="usdt">USDT</option>
                  <option value="usdc">USDC</option>
                </Select>
              </Field>
              <Button name="create" type="submit" value="1">
                Create payment order
              </Button>
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
              <li>Current plan: Monthly</li>
              <li>Renewal stacking: Allowed</li>
              <li>Grace period: 48 hours after expiry</li>
              <li>Auto-pause starts when grace ends and renewal is still unpaid.</li>
              <li><Link href="/app/strategies/grid-btc">Strategy Workspace</Link></li>
            </ul>
          </CardBody>
        </Card>
      </div>
      {newOrder ? (
        <StatusBanner
          description={`Send exactly ${selectedPlan.amount} ${selectedToken} on ${selectedChain}. Overpayment, underpayment, or wrong token will require manual review.`}
          title="Awaiting exact transfer"
          tone="warning"
        />
      ) : null}
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
                { key: "chain", label: "Chain / token" },
                { key: "amount", label: "Amount", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={rows.map((row) => ({
                ...row,
                state: <Chip tone={row.state === "Confirmed" ? "success" : "warning"}>{row.state}</Chip>,
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
