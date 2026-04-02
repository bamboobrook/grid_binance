import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Select } from "../../../components/ui/form";
import { DataTable } from "../../../components/ui/table";
import { getAdminStrategiesData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ state?: string }>;
};

export default async function AdminStrategiesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const stateFilter = typeof params.state === "string" ? params.state.toLowerCase() : "all";
  const data = await getAdminStrategiesData();
  const items = data.items.filter((item) => (stateFilter === "all" ? true : item.status.toLowerCase() === stateFilter));
  const focused = items[0] ?? null;

  return (
    <>
      <AppShellSection
        description="Operator-visible strategy data is read from backend strategy storage."
        eyebrow="Strategy supervision"
        title="Strategy Oversight"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Filter strategies</CardTitle>
              <CardDescription>Filter backend strategies by runtime state.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/strategies" method="get">
                <Field label="Runtime state">
                  <Select defaultValue={stateFilter} name="state">
                    <option value="all">All states</option>
                    <option value="draft">Draft</option>
                    <option value="running">Running</option>
                    <option value="paused">Paused</option>
                    <option value="errorpaused">ErrorPaused</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">Apply filters</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Runtime overview</CardTitle>
              <CardDescription>Focused strategy runtime summary for operators.</CardDescription>
            </CardHeader>
            <CardBody>
              {focused ? (
                <ul className="text-list">
                  <li>Name: {focused.name}</li>
                  <li>Owner: {focused.owner_email}</li>
                  <li>Focused order count: {focused.runtime.orders.length}</li>
                  <li>Focused pre-flight: {focused.runtime.last_preflight ? (focused.runtime.last_preflight.ok ? "ok" : "failed") : "missing"}</li>
                </ul>
              ) : (
                <p>No operator-visible strategies yet.</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Strategy inventory</CardTitle>
          <CardDescription>All operator-visible backend strategies.</CardDescription>
        </CardHeader>
        <CardBody>
          {items.length === 0 ? (
            <p>No operator-visible strategies yet.</p>
          ) : (
            <DataTable
              columns={[
                { key: "name", label: "Name" },
                { key: "owner", label: "Owner" },
                { key: "symbol", label: "Symbol" },
                { key: "status", label: "Status" },
                { key: "orders", label: "Active orders" },
                { key: "preflight", label: "Last pre-flight" },
              ]}
              rows={items.map((item) => ({
                id: item.id,
                name: item.name,
                orders: String(item.runtime.orders.length),
                owner: item.owner_email,
                preflight: item.runtime.last_preflight ? (item.runtime.last_preflight.ok ? "ok" : "failed") : "missing",
                status: item.status,
                symbol: item.symbol,
              }))}
            />
          )}
        </CardBody>
      </Card>
    </>
  );
}
