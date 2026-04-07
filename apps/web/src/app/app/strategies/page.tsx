import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  searchParams?: Promise<{ notice?: string | string[]; error?: string | string[]; status?: string | string[]; symbol?: string | string[] }>;
};

type StrategyListResponse = {
  items: Array<{
    budget: string;
    id: string;
    market: string;
    name: string;
    status: string;
    symbol: string;
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function StrategiesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const notice = firstValue(params.notice);
  const error = firstValue(params.error);
  const statusFilter = firstValue(params.status) ?? "all";
  const symbolFilter = firstValue(params.symbol) ?? "";
  const { items: strategies, error: loadError } = await fetchStrategies();
  const filteredStrategies = strategies.filter((item) => {
    const statusMatches = statusFilter === "all" || item.status === statusFilter;
    const symbolMatches = !symbolFilter.trim()
      || item.symbol.toLowerCase().includes(symbolFilter.trim().toLowerCase())
      || item.name.toLowerCase().includes(symbolFilter.trim().toLowerCase());
    return statusMatches && symbolMatches;
  });
  const summaries = [
    { label: "Drafts", value: String(strategies.filter((item) => item.status === "Draft").length) },
    { label: "Running", value: String(strategies.filter((item) => item.status === "Running").length) },
    { label: "Paused", value: String(strategies.filter((item) => item.status === "Paused").length) },
    { label: "Error-paused", value: String(strategies.filter((item) => item.status === "ErrorPaused").length) },
  ];

  return (
    <>
      <StatusBanner
        description="Batch start, batch pause, batch delete, and global stop-all stay visible while each strategy still owns its own edit and pre-flight flow."
        title="Lifecycle guardrails"
        tone="warning"
      />
      {notice ? <StatusBanner description={formatNotice(notice)} title="Strategy updated" tone="success" /> : null}
      {error ? <StatusBanner description={error} title="Strategy action failed" tone="warning" /> : null}
      {loadError ? <StatusBanner description={loadError} title="Strategy catalog unavailable" tone="warning" /> : null}
      <AppShellSection
        actions={
          <div className="button-row">
            <form action="/api/user/strategies/batch" method="post">
              <input name="intent" type="hidden" value="stop-all" />
              <Button type="submit">Stop all</Button>
            </form>
            <Link className="button button--ghost" href="/app/strategies/new">
              New strategy
            </Link>
          </div>
        }
        description="Review drafts, running instances, and batch lifecycle actions from backend strategy data."
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
            <CardDescription>Rows come from the backend strategies API. You can now filter first, then batch-operate on the visible result set or the manually selected rows.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/app/strategies" method="get">
              <Field label="Status filter">
                <Select defaultValue={statusFilter} name="status">
                  <option value="all">All</option>
                  <option value="Draft">Draft</option>
                  <option value="Running">Running</option>
                  <option value="Paused">Paused</option>
                  <option value="ErrorPaused">ErrorPaused</option>
                  <option value="Stopped">Stopped</option>
                </Select>
              </Field>
              <Field label="Symbol or strategy filter">
                <Input defaultValue={symbolFilter} name="symbol" />
              </Field>
              <Button type="submit">Apply filter</Button>
            </FormStack>
            <form action="/api/user/strategies/batch" method="post">
              <DataTable
                columns={[
                  { key: "pick", label: "Select" },
                  { key: "name", label: "Strategy" },
                  { key: "market", label: "Market" },
                  { key: "state", label: "State" },
                  { key: "exposure", label: "Exposure", align: "right" },
                ]}
                rows={filteredStrategies.map((row) => ({
                  id: row.id,
                  pick: <input aria-label={`Select ${row.name}`} name="ids" type="checkbox" value={row.id} />,
                  name: <Link href={`/app/strategies/${row.id}`}>{row.name}</Link>,
                  market: row.market,
                  exposure: row.budget,
                  state: (
                    <Chip tone={row.status === "Running" ? "success" : row.status === "Paused" ? "warning" : row.status === "ErrorPaused" ? "danger" : "info"}>
                      {row.status.replaceAll("_", " ")}
                    </Chip>
                  ),
                }))}
              />
              <div className="button-row">
                <Button name="intent" type="submit" value="start">Start selected</Button>
                <Button name="intent" tone="secondary" type="submit" value="pause">Pause selected</Button>
                <Button name="intent" tone="secondary" type="submit" value="delete">Delete selected</Button>
              </div>
            </form>
            <form action="/api/user/strategies/batch" method="post">
              {filteredStrategies.map((row) => (
                <input key={`filtered-${row.id}`} name="ids" type="hidden" value={row.id} />
              ))}
              <div className="button-row">
                <Button name="intent" type="submit" value="start">Start filtered</Button>
                <Button name="intent" tone="secondary" type="submit" value="pause">Pause filtered</Button>
                <Button name="intent" tone="secondary" type="submit" value="delete">Delete filtered</Button>
              </div>
            </form>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Lifecycle rules</CardTitle>
            <CardDescription>These rules are enforced in the backend-backed flows.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Strategy creation begins in draft state.</li>
              <li>Filtered result sets can be batch-started, batch-paused, or batch-deleted.</li>
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

async function fetchStrategies(): Promise<{ items: StrategyListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { items: [], error: null };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: [], error: "Strategy catalog is temporarily unavailable." };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function formatNotice(value: string) {
  const parts = value.split("-");
  if (parts.length === 0) {
    return value;
  }
  const first = parts[0] === "preflight" ? "Pre-flight" : parts[0].charAt(0).toUpperCase() + parts[0].slice(1);
  return [first, ...parts.slice(1)].join(" ");
}
