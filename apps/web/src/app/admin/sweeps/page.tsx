import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminSweepsSnapshot } from "../../../lib/api/server";

export default async function AdminSweepsPage() {
  const snapshot = await getAdminSweepsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented sweep route is present with the shared table and card system."
        eyebrow="Treasury movement"
        title="Sweeps"
      >
        <Card>
          <CardHeader>
            <CardTitle>Sweep queue</CardTitle>
            <CardDescription>All sweep actions must remain audited.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "wallet", label: "Wallet" },
                { key: "amount", label: "Amount" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={snapshot.rows}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
