import { notFound } from "next/navigation";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { DialogFrame } from "../../../../components/ui/dialog";
import { DataTable } from "../../../../components/ui/table";
import { Tabs } from "../../../../components/ui/tabs";
import { getStrategyDetailSnapshot } from "../../../../lib/api/server";

export default async function StrategyDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const snapshot = await getStrategyDetailSnapshot(id);

  if (!snapshot) {
    notFound();
  }

  return (
    <>
      <AppShellSection
        actions={<Tabs activeHref={`/app/strategies/${id}`} items={snapshot.tabs} />}
        description={snapshot.description}
        eyebrow="Strategy workspace"
        title={snapshot.title}
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.stats.map((item) => (
            <Card key={item.label}>
              <CardHeader>
                <CardTitle>{item.value}</CardTitle>
                <CardDescription>{item.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Grid ladder</CardTitle>
            <CardDescription>Per-grid take-profit ranges are visible before full runtime wiring.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "level", label: "Level" },
                { key: "range", label: "Range" },
                { key: "allocation", label: "Allocation" },
                { key: "tp", label: "Take profit", align: "right" },
              ]}
              rows={snapshot.rows}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description="When trailing take profit is enabled, maker-style TP orders are replaced with taker market close behavior and fee risk increases."
          title="Trailing TP warning"
          tone="warning"
        />
      </div>
    </>
  );
}
