import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const summaries = [
  { label: "Drafts", value: "2" },
  { label: "Running", value: "5" },
  { label: "Paused", value: "1" },
  { label: "Error-paused", value: "1" },
];

const rows = [
  { id: "grid-btc", name: "BTC Recovery Ladder", market: "Spot", state: "Running", exposure: "5,000 USDT" },
  { id: "grid-eth", name: "ETH Short Ladder", market: "USDⓈ-M", state: "Draft", exposure: "2,400 USDT" },
  { id: "grid-sol", name: "SOL Neutral Swing", market: "COIN-M", state: "Paused", exposure: "1,800 USD" },
];

export default function StrategiesPage() {
  return (
    <>
      <StatusBanner
        description="Batch actions and lifecycle guardrails stay visible while each strategy still owns its own edit and pre-flight flow."
        title="Lifecycle guardrails"
        tone="warning"
      />
      <AppShellSection
        actions={
          <Link className="button" href="/app/strategies/new">
            New strategy
          </Link>
        }
        description="Review drafts, running instances, and pause-before-edit rules in one inventory view."
        eyebrow="Strategy catalog"
        title="Strategies"
      >
        <div className="content-grid content-grid--metrics">
          {summaries.map((summary) => (
            <Card key={summary.label}>
              <CardHeader>
                <CardTitle>{summary.value}</CardTitle>
                <CardDescription>{summary.label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Strategy inventory</CardTitle>
            <CardDescription>Futures same-direction collisions remain blocked at pre-flight and runtime.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "name", label: "Strategy" },
                { key: "market", label: "Market" },
                { key: "state", label: "State" },
                { key: "exposure", label: "Exposure", align: "right" },
              ]}
              rows={rows.map((row) => ({
                ...row,
                name: row.id === "grid-btc" ? <Link href="/app/strategies/grid-btc">{row.name}</Link> : row.name,
                state: (
                  <Chip tone={row.state === "Running" ? "success" : row.state === "Paused" ? "warning" : "info"}>
                    {row.state}
                  </Chip>
                ),
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Lifecycle rules</CardTitle>
            <CardDescription>These rules are enforced in the draft and detail flows.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Strategy creation begins in draft state.</li>
              <li>Edits require pause first and save before restart.</li>
              <li>Delete is allowed only when working orders and positions are both cleared.</li>
              <li>Runtime exceptions auto-pause the affected strategy and trigger Telegram alerts.</li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
