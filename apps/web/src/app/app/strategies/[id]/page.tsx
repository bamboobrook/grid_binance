import Link from "next/link";
import { notFound } from "next/navigation";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Chip } from "../../../../components/ui/chip";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { DataTable } from "../../../../components/ui/table";
import { firstValue } from "../../../../lib/auth";

type StrategyDetailPageProps = {
  params: Promise<{ id: string }>;
  searchParams?: Promise<{
    draft?: string | string[];
    edited?: string | string[];
    generation?: string | string[];
    marketType?: string | string[];
    mode?: string | string[];
    name?: string | string[];
    preflight?: string | string[];
    started?: string | string[];
    symbol?: string | string[];
    trailing?: string | string[];
  }>;
};

const ladderRows = [
  { id: "level-1", level: "L1", range: "86,000 - 86,750", allocation: "0.008 BTC", tp: "1.2%" },
  { id: "level-2", level: "L2", range: "86,750 - 87,500", allocation: "0.007 BTC", tp: "1.1%" },
  { id: "level-3", level: "L3", range: "87,500 - 88,250", allocation: "0.006 BTC", tp: "0.9%" },
];

const preflightRows = [
  { id: "check-1", item: "Exchange filters", result: "Pass" },
  { id: "check-2", item: "Balance coverage", result: "Pass" },
  { id: "check-3", item: "Hedge mode", result: "Pass" },
];

export default async function StrategyDetailPage({ params, searchParams }: StrategyDetailPageProps) {
  const { id } = await params;

  if (id !== "grid-btc") {
    notFound();
  }

  const query = (await searchParams) ?? {};
  const strategyName = firstValue(query.name) ?? "BTC Recovery Ladder";
  const symbol = firstValue(query.symbol) ?? "BTCUSDT";
  const marketType = firstValue(query.marketType) ?? "spot";
  const mode = firstValue(query.mode) ?? "classic";
  const generation = firstValue(query.generation) ?? "geometric";
  const trailing = firstValue(query.trailing) ?? "0.8";
  const draft = firstValue(query.draft) === "1";
  const edited = firstValue(query.edited) === "1";
  const preflight = firstValue(query.preflight) === "1";
  const started = firstValue(query.started) === "1";
  const stateLabel = started ? "Running" : preflight ? "Ready to launch" : draft ? "Draft" : "Paused";

  return (
    <>
      {draft ? <StatusBanner description="Draft saved and ready for parameter review." title="Draft saved" tone="success" /> : null}
      {edited ? <StatusBanner description="Edits saved. Runtime parameters are now queued for the next pre-flight." title="Edits saved" tone="success" /> : null}
      {preflight ? (
        <StatusBanner
          description="Exchange filters, balance, and hedge-mode checks passed."
          title="Pre-flight passed"
          tone="success"
        />
      ) : null}
      {started ? (
        <StatusBanner
          description="Strategy started with the saved parameters and the current market snapshot."
          title="Strategy started"
          tone="success"
        />
      ) : null}
      <AppShellSection
        actions={
          <div className="button-row">
            <Link className="button button--ghost" href="/app/analytics">
              Analytics
            </Link>
            <Link className="button button--ghost" href="/help/expiry-reminder">
              Help Center
            </Link>
          </div>
        }
        description="Review saved parameters, keep edit history visible, and do not start until pre-flight is green."
        eyebrow="Strategy workspace"
        title="Strategy Workspace"
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>{strategyName}</CardTitle>
              <CardDescription>{symbol}</CardDescription>
            </CardHeader>
            <CardBody>{marketType === "spot" ? "Spot grid" : marketType}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{mode}</CardTitle>
              <CardDescription>Mode</CardDescription>
            </CardHeader>
            <CardBody>Generation: {generation}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{trailing}%</CardTitle>
              <CardDescription>Trailing take profit</CardDescription>
            </CardHeader>
            <CardBody>Use only when taker execution fee tradeoff is acceptable.</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>
                <Chip tone={started ? "success" : preflight ? "info" : "warning"}>{stateLabel}</Chip>
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
            <CardDescription>The same form handles save, pre-flight, and start so the visible state stays testable.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack method="get">
              {draft ? <input name="draft" type="hidden" value="1" /> : null}
              {edited ? <input name="edited" type="hidden" value="1" /> : null}
              {preflight ? <input name="preflight" type="hidden" value="1" /> : null}
              {started ? <input name="started" type="hidden" value="1" /> : null}
              <Field label="Strategy name">
                <Input defaultValue={strategyName} name="name" required />
              </Field>
              <Field label="Symbol">
                <Input defaultValue={symbol} name="symbol" required />
              </Field>
              <Field label="Market type">
                <Select defaultValue={marketType} name="marketType">
                  <option value="spot">Spot</option>
                  <option value="usd-m">USDⓈ-M futures</option>
                  <option value="coin-m">COIN-M futures</option>
                </Select>
              </Field>
              <Field label="Trailing take profit (%)">
                <Input defaultValue={trailing} inputMode="decimal" name="trailing" />
              </Field>
              <Field label="Post-trigger behavior">
                <Select defaultValue="rebuild" name="postTrigger">
                  <option value="stop">Stop after execution</option>
                  <option value="rebuild">Rebuild and continue</option>
                </Select>
              </Field>
              <ButtonRow>
                <Button name="edited" type="submit" value="1">
                  Save edits
                </Button>
                <Button name="preflight" tone="secondary" type="submit" value="1">
                  Run pre-flight
                </Button>
                <Button name="started" type="submit" value="1">
                  Start strategy
                </Button>
              </ButtonRow>
            </FormStack>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Pre-flight checklist</CardTitle>
            <CardDescription>Start requires all checks to pass and any failures should explain the exact blocker.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "item", label: "Check" },
                { key: "result", label: "Result", align: "right" },
              ]}
              rows={preflightRows.map((row) => ({
                ...row,
                result: <Chip tone="success">{row.result}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Grid ladder</CardTitle>
            <CardDescription>Per-grid take-profit ranges stay visible for manual review and future exports.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "level", label: "Level" },
                { key: "range", label: "Range" },
                { key: "allocation", label: "Allocation" },
                { key: "tp", label: "Take profit", align: "right" },
              ]}
              rows={ladderRows}
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
