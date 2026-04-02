import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminAddressPoolsSnapshot } from "../../../lib/api/server";

export default async function AdminAddressPoolsPage() {
  const snapshot = await getAdminAddressPoolsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="Pool sizing, queue pressure, and sweep-adjacent operations share a reusable admin frame."
        eyebrow="Chain billing ops"
        title="Address Pools"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Current pool pressure</CardTitle>
              <CardDescription>One order gets one assigned address for one hour.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "chain", label: "Chain" },
                  { key: "total", label: "Total addresses", align: "right" },
                  { key: "locked", label: "Locked", align: "right" },
                  { key: "queue", label: "Queue", align: "right" },
                ]}
                rows={snapshot.rows}
              />
            </CardBody>
          </Card>
          <DialogFrame
            description="If the address pool is full, new payment orders must queue instead of reusing a locked address."
            title="Pool saturation rule"
            tone="warning"
          />
        </div>
      </AppShellSection>
    </>
  );
}
