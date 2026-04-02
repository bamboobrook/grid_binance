import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getAnalyticsSnapshot } from "../../../lib/api/server";

export default async function AnalyticsPage() {
  const snapshot = await getAnalyticsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/app/analytics" items={snapshot.tabs} />}
        description="Cards, tabs, and tables here define the shared reporting surface before real analytics endpoints are connected."
        eyebrow="Reporting"
        title="Analytics"
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.metrics.map((metric) => (
            <Card key={metric.label}>
              <CardHeader>
                <CardTitle>{metric.value}</CardTitle>
                <CardDescription>{metric.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Export surfaces</CardTitle>
          <CardDescription>CSV export remains a mandatory V1 capability for orders, fills, statistics, and payments.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "export", label: "Export" },
              { key: "cadence", label: "Cadence" },
              { key: "scope", label: "Scope" },
            ]}
            rows={snapshot.rows}
          />
        </CardBody>
      </Card>
    </>
  );
}
