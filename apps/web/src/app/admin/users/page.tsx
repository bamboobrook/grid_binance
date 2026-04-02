import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminUsersData } from "../../../lib/api/admin-product-state";

function membershipLabel(status: string | null) {
  return status ?? "No membership";
}

export default async function AdminUsersPage() {
  const data = await getAdminUsersData();

  return (
    <>
      <AppShellSection
        description="Identity-backed user directory with membership and billing overlays. No membership record users remain visible here."
        eyebrow="User operations"
        title="User Management"
      >
        <Card>
          <CardHeader>
            <CardTitle>User inventory</CardTitle>
            <CardDescription>Registered users first, with admin-created commercial records folded into the same directory.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "email", label: "Email" },
                { key: "registration", label: "Registration" },
                { key: "membership", label: "Membership" },
                { key: "order", label: "Latest order" },
                { key: "role", label: "Admin role" },
              ]}
              rows={data.items.map((item) => ({
                email: item.email,
                id: item.email,
                membership: membershipLabel(item.membership?.status ?? null),
                order: item.latest_order_status ?? "-",
                registration: item.registered
                  ? item.email_verified
                    ? "Registered · verified"
                    : "Registered · pending verification"
                  : "Commercial record only",
                role: item.admin_role ?? "user",
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
