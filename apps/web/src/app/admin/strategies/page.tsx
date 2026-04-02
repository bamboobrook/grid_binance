import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminStrategiesSnapshot } from "../../../lib/api/server";

export default async function AdminStrategiesPage() {
  const snapshot = await getAdminStrategiesSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented admin strategy route now exists inside the shared admin shell."
        eyebrow="Strategy supervision"
        title="Strategies"
      >
        <Card>
          <CardHeader>
            <CardTitle>Runtime overview</CardTitle>
            <CardDescription>Detailed runtime control remains later work.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "user", label: "User" },
                { key: "strategy", label: "Strategy" },
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
