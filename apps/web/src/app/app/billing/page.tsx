import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, Field, FormStack, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

const planCatalog = [
  { label: "Monthly", amount: "20.00 USD equivalent" },
  { label: "Quarterly", amount: "54.00 USD equivalent" },
  { label: "Yearly", amount: "180.00 USD equivalent" },
];

export default async function BillingPage() {
  const state = await getCurrentUserProductState();

  return (
    <>
      <StatusBanner
        description="Membership enters a 48-hour grace period after expiry. Existing strategies may continue only during that window."
        title="Grace-period reminder enabled"
        tone="warning"
      />
      {state.flash.billing ? (
        <StatusBanner description={state.flash.billing} title="Awaiting exact transfer" tone="warning" />
      ) : null}
      <AppShellSection
        description="Create renewal orders with visible exact-amount warnings, plan pricing, and membership timing."
        eyebrow="Membership billing"
        title="Billing Center"
      >
        <div className="content-grid content-grid--metrics">
          {planCatalog.map((item) => (
            <Card key={item.label}>
              <CardHeader>
                <CardTitle>{item.label}</CardTitle>
                <CardDescription>{item.amount}</CardDescription>
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
            <p>Next renewal: {state.billing.nextRenewalAt}</p>
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
              <li>Membership status: {state.billing.membershipStatus}</li>
              {state.billing.membershipStatus === "Unknown" ? <li>Entitlement truth is temporarily unavailable; strategy starts remain blocked.</li> : null}
              <li>Current plan: {state.billing.currentPlan}</li>
              <li>Renewal stacking: Allowed</li>
              <li>Grace period ends: {state.billing.graceEndsAt}</li>
              <li><Link href={`/app/strategies/${state.strategies[0]?.id ?? ""}`}>Strategy Workspace</Link></li>
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
                { key: "amount", label: "Amount", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={state.billing.orders.map((row) => ({
                id: row.id,
                order: row.order,
                chainToken: `${row.chain} / ${row.token}`,
                amount: row.amount,
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
