import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getTelegramSnapshot } from "../../../lib/api/server";

export default async function TelegramPage() {
  const snapshot = await getTelegramSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented Telegram route now lives in the shared shell instead of the older notification page-map variant."
        eyebrow="Telegram bind"
        title="Telegram"
      >
        <div className="content-grid content-grid--metrics">
          {snapshot.channels.map((channel) => (
            <Card key={channel.label}>
              <CardHeader>
                <CardTitle>{channel.value}</CardTitle>
                <CardDescription>{channel.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Recent delivery events</CardTitle>
          <CardDescription>One user binds one Telegram identity only.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "event", label: "Event" },
              { key: "channel", label: "Channel" },
              { key: "state", label: "State", align: "right" },
            ]}
            rows={snapshot.rows.map((row) => ({
              ...row,
              state: <Chip tone={row.state === "Delivered" ? "success" : "warning"}>{row.state}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
