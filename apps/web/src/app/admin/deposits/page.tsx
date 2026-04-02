import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminDepositsSnapshot } from "../../../lib/api/server";

export default async function AdminDepositsPage() {
  const snapshot = await getAdminDepositsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented deposits route now holds the abnormal-payment shell surface."
        eyebrow="Deposit review"
        title="Deposits"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Abnormal deposit queue</CardTitle>
              <CardDescription>Exact-amount mismatches stay blocked for manual review.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "order", label: "Order" },
                  { key: "issue", label: "Issue" },
                  { key: "amount", label: "Amount", align: "right" },
                  { key: "action", label: "Action", align: "right" },
                ]}
                rows={snapshot.rows}
              />
            </CardBody>
          </Card>
          <DialogFrame
            description="Overpayment, underpayment, wrong token, and abnormal transfer must be held for manual handling."
            title="Manual handling rule"
            tone="danger"
          />
        </div>
      </AppShellSection>
    </>
  );
}
