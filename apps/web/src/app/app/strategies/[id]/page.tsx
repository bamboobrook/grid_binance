import Link from "next/link";
import { notFound } from "next/navigation";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Chip } from "../../../../components/ui/chip";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { DataTable } from "../../../../components/ui/table";
import { findStrategy, getCurrentUserProductState } from "../../../../lib/api/user-product-state";

export default async function StrategyDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const state = await getCurrentUserProductState();
  const strategy = findStrategy(state, id);

  if (!strategy) {
    notFound();
  }

  return (
    <>
      {state.flash.strategy ? (
        <StatusBanner
          description={strategy.preflightMessage ?? "The latest strategy action has been recorded in the workspace."}
          title={state.flash.strategy}
          tone={state.flash.strategy.includes("failed") || state.flash.strategy.includes("blocked") ? "warning" : "success"}
        />
      ) : null}
      <AppShellSection
        actions={
          <div className="button-row">
            <Link className="button button--ghost" href="/app/orders">
              Orders
            </Link>
            <Link className="button button--ghost" href="/app/help">
              Help Center
            </Link>
          </div>
        }
        description="Review saved parameters, independent strategy statistics, and pre-flight status before restarting or launching."
        eyebrow="Strategy workspace"
        title="Strategy Workspace"
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>{strategy.name}</CardTitle>
              <CardDescription>{strategy.symbol}</CardDescription>
            </CardHeader>
            <CardBody>{strategy.marketType}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{strategy.mode}</CardTitle>
              <CardDescription>Mode</CardDescription>
            </CardHeader>
            <CardBody>Generation: {strategy.generation}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{strategy.trailingTakeProfit}%</CardTitle>
              <CardDescription>Trailing take profit</CardDescription>
            </CardHeader>
            <CardBody>Use only when taker execution fee tradeoff is acceptable.</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>
                <Chip tone={strategy.status === "running" ? "success" : strategy.preflightStatus === "passed" ? "info" : "warning"}>
                  {strategy.status.replaceAll("_", " ")}
                </Chip>
              </CardTitle>
              <CardDescription>Current state</CardDescription>
            </CardHeader>
            <CardBody>Pause before edit, save before restart, no hot-modify while running.</CardBody>
          </Card>
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Edit and start flow</CardTitle>
            <CardDescription>Pre-flight and start are POST-backed lifecycle steps instead of query-flag shortcuts.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action={`/api/user/strategies/${strategy.id}`} method="post">
              <Field label="Strategy name">
                <Input defaultValue={strategy.name} name="name" required />
              </Field>
              <Field label="Symbol">
                <Input defaultValue={strategy.symbol} name="symbol" required />
              </Field>
              <Field label="Market type">
                <Select defaultValue={strategy.marketType} name="marketType">
                  <option value="spot">spot</option>
                  <option value="usd-m">usd-m</option>
                  <option value="coin-m">coin-m</option>
                </Select>
              </Field>
              <Field label="Trailing take profit (%)">
                <Input defaultValue={strategy.trailingTakeProfit} inputMode="decimal" name="trailing" />
              </Field>
              <Field label="Post-trigger behavior">
                <Select defaultValue={strategy.postTrigger} name="postTrigger">
                  <option value="stop">Stop after execution</option>
                  <option value="rebuild">Rebuild and continue</option>
                </Select>
              </Field>
              <ButtonRow>
                <Button name="intent" type="submit" value="save">
                  Save edits
                </Button>
                <Button name="intent" tone="secondary" type="submit" value="preflight">
                  Run pre-flight
                </Button>
                <Button name="intent" type="submit" value="start">
                  Start strategy
                </Button>
              </ButtonRow>
            </FormStack>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Pre-flight checklist</CardTitle>
            <CardDescription>Start requires all checks to pass and any failures explain the exact blocker.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "item", label: "Check" },
                { key: "result", label: "Result", align: "right" },
              ]}
              rows={strategy.preflightChecks.map((row) => ({
                ...row,
                result: <Chip tone={row.result === "Pass" ? "success" : "danger"}>{row.result}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--metrics">
        {[
          ["Realized PnL", strategy.realizedPnl],
          ["Unrealized PnL", strategy.unrealizedPnl],
          ["Fees", strategy.fees],
          ["Funding fees", strategy.fundingFees],
          ["Net profit", strategy.netProfit],
          ["Cost basis", strategy.costBasis],
          ["Fill count", String(strategy.fillCount)],
          ["Order count", String(strategy.orderCount)],
          ["Current holdings", strategy.holdings],
        ].map(([label, value]) => (
          <Card key={label}>
            <CardHeader>
              <CardTitle>{value}</CardTitle>
              <CardDescription>{label}</CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Grid ladder</CardTitle>
            <CardDescription>Per-grid take-profit ranges stay visible for manual review and export readiness.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "level", label: "Level" },
                { key: "range", label: "Range" },
                { key: "allocation", label: "Allocation" },
                { key: "tp", label: "Take profit", align: "right" },
              ]}
              rows={strategy.gridLevels}
            />
          </CardBody>
        </Card>
        <DialogFrame
          description="Running strategy parameters cannot be hot-modified. Trailing take profit uses taker execution and may increase fees."
          title="Running strategy parameters cannot be hot-modified"
          tone="warning"
        />
      </div>
    </>
  );
}
