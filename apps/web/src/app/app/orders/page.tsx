import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

export default async function OrdersPage() {
  const state = await getCurrentUserProductState();
  const orderRows = state.strategies.map((strategy) => ({
    id: `${strategy.id}-orders`,
    order: `ORD-${strategy.id.slice(0, 4).toUpperCase()}`,
    symbol: strategy.symbol,
    side: strategy.mode,
    state: strategy.status === "running" ? "Working" : strategy.preflightStatus === "passed" ? "Ready" : strategy.status === "error_paused" ? "Blocked" : "Draft",
  }));

  return (
    <>
      <StatusBanner
        description="Strategy orders, fill history, and exchange account activity now come from the same user state and account activity flow."
        title="Orders and history"
        tone="info"
      />
      <AppShellSection
        description="Use this route to review working orders, fills, and exchange-side activity without leaving the user shell."
        eyebrow="User orders"
        title="Orders & History"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Strategy orders</CardTitle>
              <CardDescription>Order rows are derived from the current strategy state and lifecycle posture.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "order", label: "Order" },
                  { key: "symbol", label: "Symbol" },
                  { key: "side", label: "Side" },
                  { key: "state", label: "State", align: "right" },
                ]}
                rows={orderRows.map((row) => ({
                  ...row,
                  state: <Chip tone={row.state === "Working" ? "success" : row.state === "Ready" ? "info" : row.state === "Blocked" ? "danger" : "warning"}>{row.state}</Chip>,
                }))}
              />
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Fill history</CardTitle>
              <CardDescription>Per-fill profit context comes directly from the user-state reporting surface.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "state", label: "Event" },
                  { key: "symbol", label: "Symbol" },
                  { key: "pnl", label: "PnL", align: "right" },
                ]}
                rows={state.recentFills.map((row) => ({
                  id: row.id,
                  state: row.state === "Trailing TP" ? "Trailing TP exit" : row.state === "Settled" ? "Grid fill" : row.state,
                  symbol: row.symbol,
                  pnl: row.pnl,
                }))}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Exchange account activity</CardTitle>
          <CardDescription>Visibility into account-level history helps platform analytics and reconciliation.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "at", label: "Timestamp" },
              { key: "activity", label: "Activity" },
              { key: "detail", label: "Detail", align: "right" },
            ]}
            rows={state.tradeHistory}
          />
        </CardBody>
      </Card>
    </>
  );
}
