import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getUserDashboardSnapshot } from "../../../lib/api/server";

export default async function DashboardPage() {
  const snapshot = await getUserDashboardSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/app/dashboard" items={snapshot.tabs} />}
        description="Key wallet, PnL, membership, and runtime signals stay consistent across the shared app shell."
        eyebrow="User overview"
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
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Recent fills</CardTitle>
            <CardDescription>Per-fill profit context for web and Telegram surfaces.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "symbol", label: "Symbol" },
                { key: "side", label: "Side" },
                { key: "pnl", label: "PnL", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={snapshot.fills.map((fill) => ({
                ...fill,
                state: <Chip tone={fill.state === "Trailing TP" ? "warning" : "success"}>{fill.state}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Context notes</CardTitle>
            <CardDescription>Shell task stops short of full business-detail pages.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {snapshot.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
