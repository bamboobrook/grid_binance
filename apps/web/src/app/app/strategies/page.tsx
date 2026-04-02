import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getStrategiesSnapshot } from "../../../lib/api/server";

export default async function StrategiesPage() {
  const snapshot = await getStrategiesSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/app/strategies" items={snapshot.tabs} />}
        description="The shell provides a stable frame for strategy state, lifecycle warnings, and future batch actions."
        eyebrow="Strategy catalog"
        title="Strategies"
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.summaries.map((summary) => (
            <Card key={summary.label}>
              <CardHeader>
                <CardTitle>{summary.value}</CardTitle>
                <CardDescription>{summary.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Current strategy inventory</CardTitle>
          <CardDescription>Full batch start/pause/delete workflows land in later task content.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "name", label: "Strategy" },
              { key: "market", label: "Market" },
              { key: "state", label: "State" },
              { key: "exposure", label: "Exposure", align: "right" },
            ]}
            rows={snapshot.rows.map((row) => ({
              ...row,
              name: row.id === "grid-btc" ? <Link href="/app/strategies/grid-btc">{row.name}</Link> : row.name,
              state: (
                <Chip tone={row.state === "Running" ? "success" : row.state === "Paused" ? "warning" : "info"}>
                  {row.state}
                </Chip>
              ),
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
