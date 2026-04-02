import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminMembershipsSnapshot } from "../../../lib/api/server";

export default async function AdminMembershipsPage() {
  const snapshot = await getAdminMembershipsSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented admin memberships route now exists within the shared shell system."
        eyebrow="Membership operations"
        title="Memberships"
      >
        <Card>
          <CardHeader>
            <CardTitle>Membership queue</CardTitle>
            <CardDescription>Manual override workflows arrive later; this task establishes the shell route.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "user", label: "User" },
                { key: "plan", label: "Plan" },
                { key: "state", label: "State" },
                { key: "action", label: "Next action", align: "right" },
              ]}
              rows={snapshot.rows}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
