import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getOrdersSnapshot } from "../../../lib/api/server";

export default async function OrdersPage() {
  const snapshot = await getOrdersSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented order route is now anchored in the shared user shell and table system."
        eyebrow="User orders"
        title="Orders"
      >
        <Card>
          <CardHeader>
            <CardTitle>Recent orders</CardTitle>
            <CardDescription>Full export and filtering logic lands in later tasks.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "order", label: "Order" },
                { key: "symbol", label: "Symbol" },
                { key: "side", label: "Side" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={snapshot.rows.map((row) => ({
                ...row,
                state: <Chip tone={row.state === "Filled" ? "success" : row.state === "Working" ? "info" : "warning"}>{row.state}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
