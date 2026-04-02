import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getNotificationsSnapshot } from "../../../lib/api/server";

export default async function NotificationsPage() {
  const snapshot = await getNotificationsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="Notification channels share the same visual system used for runtime, billing, and admin warnings."
        eyebrow="Delivery surfaces"
        title="Notifications"
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
          <CardTitle>Recent notification events</CardTitle>
          <CardDescription>Web inbox and Telegram share the same event taxonomy.</CardDescription>
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
