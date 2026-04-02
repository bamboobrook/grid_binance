import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getAdminDashboardSnapshot } from "../../../lib/api/server";

export default async function AdminDashboardPage() {
  const snapshot = await getAdminDashboardSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/admin/dashboard" items={snapshot.tabs} />}
        description="Shared admin chrome keeps queue pressure, pricing, pools, templates, and audit paths in one consistent frame."
        eyebrow="Admin overview"
        title="Dashboard"
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.metrics.map((metric) => (
            <Card key={metric.label}>
              <CardHeader>
                <CardTitle>{metric.value}</CardTitle>
                <CardDescription>{metric.label}</CardDescription>
              </CardHeader>
              <CardBody>{metric.detail}</CardBody>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Recent operator activity</CardTitle>
          <CardDescription>Audit depth expands in later tasks; the shell is in place now.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "actor", label: "Actor" },
              { key: "action", label: "Action" },
              { key: "target", label: "Target" },
            ]}
            rows={snapshot.rows}
          />
        </CardBody>
      </Card>
    </>
  );
}
