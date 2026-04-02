import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Select } from "../../../components/ui/form";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

type StrategiesPageProps = {
  searchParams?: Promise<{
    state?: string;
  }>;
};

export default async function AdminStrategiesPage({ searchParams }: StrategiesPageProps) {
  const params = (await searchParams) ?? {};
  const stateFilter = typeof params.state === "string" ? params.state : "all";
  const state = await getCurrentAdminProductState();
  const strategies = state.strategies.filter((item) => (stateFilter === "all" ? true : item.state === stateFilter));

  return (
    <>
      <AppShellSection
        description="Filter runtime state, inspect operator-facing incidents, and understand why a strategy is running, paused, or error-paused."
        eyebrow="Strategy supervision"
        title="Strategy Oversight"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Filter strategies</CardTitle>
              <CardDescription>Focus on runtime incidents without leaving the strategy oversight route.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/strategies" method="get">
                <Field label="Runtime state">
                  <Select defaultValue={stateFilter} name="state">
                    <option value="all">All states</option>
                    <option value="running">Running</option>
                    <option value="paused">Paused</option>
                    <option value="error_paused">Error paused</option>
                    <option value="draft">Draft</option>
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
              <CardTitle>Runtime summary</CardTitle>
              <CardDescription>Incident text explains the exact operator follow-up.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Error paused: {state.strategies.filter((item) => item.state === "error_paused").length}</li>
                <li>Running: {state.strategies.filter((item) => item.state === "running").length}</li>
                <li>Filtered rows: {strategies.length}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Runtime overview</CardTitle>
          <CardDescription>Operator-visible strategy state, market scope, and latest incident.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "user", label: "User" },
              { key: "strategy", label: "Strategy" },
              { key: "market", label: "Market" },
              { key: "symbol", label: "Symbol" },
              { key: "state", label: "State" },
              { key: "incident", label: "Incident" },
            ]}
            rows={strategies.map((item) => ({
              id: item.id,
              incident: item.incident,
              market: item.market,
              state: <Chip tone={item.state === "running" ? "success" : item.state === "error_paused" ? "danger" : "warning"}>{item.state}</Chip>,
              strategy: item.name,
              symbol: item.symbol,
              user: item.user,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
