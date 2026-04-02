import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DialogFrame } from "../../../components/ui/dialog";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getBillingSnapshot } from "../../../lib/api/server";

export default async function BillingPage() {
  const snapshot = await getBillingSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/app/billing" items={snapshot.tabs} />}
        description="The shared shell keeps renewal, entitlement, and help surfaces aligned while billing APIs are still mocked."
        eyebrow="Membership billing"
        title="Billing Center"
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.plans.map((plan) => (
            <Card key={plan.label}>
              <CardHeader>
                <CardTitle>{plan.value}</CardTitle>
                <CardDescription>{plan.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Payment orders</CardTitle>
            <CardDescription>Exact amount and token matching remain mandatory for auto-confirmation.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: "Order" },
                { key: "chain", label: "Chain / token" },
                { key: "amount", label: "Amount", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={snapshot.rows.map((row) => ({
                ...row,
                state: <Chip tone={row.state === "Confirmed" ? "success" : "warning"}>{row.state}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description="The UI should warn users that overpayment, underpayment, wrong token, or abnormal transfer requires admin intervention."
          title="Payment amount must match exactly"
          tone="danger"
        />
      </div>
    </>
  );
}
