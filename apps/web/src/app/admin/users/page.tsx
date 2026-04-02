import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminUsersData } from "../../../lib/api/admin-product-state";

export default async function AdminUsersPage() {
  const data = await getAdminUsersData();

  return (
    <>
      <AppShellSection
        description="User summaries are read from backend membership and billing state."
        eyebrow="User operations"
        title="User Management"
      >
        <Card>
          <CardHeader>
            <CardTitle>User inventory</CardTitle>
            <CardDescription>Backend-backed user summaries.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "email", label: "Email" },
                { key: "status", label: "Membership" },
                { key: "order", label: "Latest order" },
              ]}
              rows={data.items.map((item) => ({
                id: item.email,
                email: item.email,
                order: item.latest_order_status ?? "-",
                status: item.membership.status,
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
