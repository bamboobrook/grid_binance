import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminSystemSnapshot } from "../../../lib/api/server";

export default async function AdminSystemPage() {
  const snapshot = await getAdminSystemSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented system route now exists as a shared-shell destination for future operator controls."
        eyebrow="System settings"
        title="System"
      >
        <Card>
          <CardHeader>
            <CardTitle>System configuration preview</CardTitle>
            <CardDescription>Later tasks will connect real configuration writes and audit trails.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "key", label: "Key" },
                { key: "value", label: "Value" },
                { key: "scope", label: "Scope", align: "right" },
              ]}
              rows={snapshot.rows}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
