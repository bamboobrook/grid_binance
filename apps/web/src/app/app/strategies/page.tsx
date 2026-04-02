import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

export default async function StrategiesPage() {
  const state = await getCurrentUserProductState();
  const summaries = [
    { label: "Drafts", value: String(state.strategies.filter((item) => item.status === "draft").length) },
    { label: "Running", value: String(state.strategies.filter((item) => item.status === "running").length) },
    { label: "Paused", value: String(state.strategies.filter((item) => item.status === "paused").length) },
    { label: "Error-paused", value: String(state.strategies.filter((item) => item.status === "error_paused").length) },
  ];

  return (
    <>
      <StatusBanner
        description="Batch actions and lifecycle guardrails stay visible while each strategy owns its own edit and pre-flight flow."
        title="Lifecycle guardrails"
        tone="warning"
      />
      <AppShellSection
        actions={
          <Link className="button" href="/app/strategies/new">
            New strategy
          </Link>
        }
        description="Review drafts, running instances, and pause-before-edit rules from your current user state."
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
            <CardDescription>Rows now come from user session-backed strategy state instead of fixed demo samples.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "name", label: "Strategy" },
                { key: "market", label: "Market" },
                { key: "state", label: "State" },
                { key: "exposure", label: "Exposure", align: "right" },
              ]}
              rows={state.strategies.map((row) => ({
                id: row.id,
                name: <Link href={`/app/strategies/${row.id}`}>{row.name}</Link>,
                market: row.marketType,
                exposure: row.notional,
                state: (
                  <Chip tone={row.status === "running" ? "success" : row.status === "paused" ? "warning" : row.status === "error_paused" ? "danger" : "info"}>
                    {row.status.replaceAll("_", " ")}
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
