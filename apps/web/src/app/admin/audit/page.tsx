import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getAdminAuditSnapshot } from "../../../lib/api/server";

export default async function AdminAuditPage() {
  const snapshot = await getAdminAuditSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={
          <Tabs
            activeHref="/admin/audit"
            items={[
              { href: "/admin/dashboard", label: "Overview" },
              { href: "/admin/deposits", label: "Billing" },
              { href: "/admin/audit", label: "Audit" },
            ]}
          />
        }
        description="Audit surfaces now follow the same shell, card, tabs, and table primitives as the rest of admin operations."
        eyebrow="Admin retention"
        title="Audit Logs"
      >
        <div className="content-grid content-grid--split">
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Required fields</CardTitle>
              <CardDescription>Critical actions must keep actor, timestamp, target, and context.</CardDescription>
            </CardHeader>
            <CardBody>
              Before/after summaries and source metadata will be wired into backend events in later tasks.
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Recent log lines</CardTitle>
              <CardDescription>Shared admin table baseline.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "timestamp", label: "Timestamp" },
                  { key: "actor", label: "Actor" },
                  { key: "action", label: "Action" },
                  { key: "target", label: "Target" },
                ]}
                rows={snapshot.rows}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
