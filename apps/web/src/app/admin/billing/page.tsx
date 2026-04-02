import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminBillingSnapshot } from "../../../lib/api/server";

export default async function AdminBillingPage() {
  const snapshot = await getAdminBillingSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="Abnormal billing handling now sits in the shared admin shell with a reusable warning dialog and data table."
        eyebrow="Billing operations"
        title="Billing Admin"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Abnormal order queue</CardTitle>
              <CardDescription>Orders with mismatched transfers stay blocked for manual resolution.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "order", label: "Order" },
                  { key: "issue", label: "Issue" },
                  { key: "amount", label: "Amount", align: "right" },
                  { key: "action", label: "Next action", align: "right" },
                ]}
                rows={snapshot.rows}
              />
            </CardBody>
          </Card>
          <DialogFrame
            description="Wallet sweep and treasury handling remain separate admin workflows, but abnormal-order decisions must still be audited."
            title="Manual handling rule"
            tone="danger"
          />
        </div>
      </AppShellSection>
    </>
  );
}
