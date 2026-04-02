import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const orderRows = [
  { id: "ord-1", order: "ORD-8801", symbol: "BTCUSDT", side: "Buy", state: "Filled" },
  { id: "ord-2", order: "ORD-8802", symbol: "ETHUSDT", side: "Sell", state: "Working" },
  { id: "ord-3", order: "ORD-8803", symbol: "SOLUSDT", side: "Buy", state: "Cancelled" },
];

const fillRows = [
  { id: "fill-1", event: "Trailing TP exit", symbol: "BTCUSDT", pnl: "+42.18" },
  { id: "fill-2", event: "Grid buy fill", symbol: "ETHUSDT", pnl: "+9.42" },
  { id: "fill-3", event: "Grid sell fill", symbol: "SOLUSDT", pnl: "-3.11" },
];

const historyRows = [
  { id: "hist-1", at: "2026-04-02 09:21", activity: "API credential retest", detail: "Passed" },
  { id: "hist-2", at: "2026-04-02 08:55", activity: "Billing order created", detail: "Awaiting exact transfer" },
  { id: "hist-3", at: "2026-04-01 23:14", activity: "Strategy auto-pause", detail: "Runtime anomaly surfaced" },
];

export default function OrdersPage() {
  return (
    <>
      <StatusBanner
        description="Strategy orders, fill history, and exchange account activity are now visible together for reconciliation and export readiness."
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
              <CardDescription>Working, filled, and cancelled orders stay grouped by strategy runtime.</CardDescription>
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
                  state: <Chip tone={row.state === "Filled" ? "success" : row.state === "Working" ? "info" : "warning"}>{row.state}</Chip>,
                }))}
              />
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Fill history</CardTitle>
              <CardDescription>Per-fill profit context supports export and Telegram notifications.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "event", label: "Event" },
                  { key: "symbol", label: "Symbol" },
                  { key: "pnl", label: "PnL", align: "right" },
                ]}
                rows={fillRows}
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
            rows={historyRows}
          />
        </CardBody>
      </Card>
    </>
  );
}
