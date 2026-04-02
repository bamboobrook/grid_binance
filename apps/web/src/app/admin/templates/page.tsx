import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getAdminTemplatesSnapshot } from "../../../lib/api/server";

export default async function AdminTemplatesPage() {
  const snapshot = await getAdminTemplatesSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={
          <Tabs
            activeHref="/admin/templates"
            items={[
              { href: "/admin/templates", label: "Templates" },
              { href: "/admin/dashboard", label: "Overview" },
              { href: "/admin/audit", label: "Audit" },
            ]}
          />
        }
        description="Template CRUD is still ahead, but the shared admin shell now holds the page structure and warnings."
        eyebrow="Strategy templates"
        title="Templates"
      >
        <div className="content-grid content-grid--split">
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Copy semantics</CardTitle>
              <CardDescription>Applied templates become user-owned configs.</CardDescription>
            </CardHeader>
            <CardBody>
              Later template edits must not rewrite previously applied user strategies.
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Template inventory</CardTitle>
              <CardDescription>Preview of the shared table style for admin assets.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "template", label: "Template" },
                  { key: "market", label: "Market" },
                  { key: "usage", label: "Usage", align: "right" },
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
