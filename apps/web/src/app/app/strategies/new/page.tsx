import { cookies } from "next/headers";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  searchParams?: Promise<{ error?: string | string[]; symbolQuery?: string | string[] }>;
};

type StrategyListResponse = { items: Array<{ id: string }> };

type SymbolSearchResponse = {
  items: Array<{ base_asset: string; market: string; quote_asset: string; symbol: string }>;
};

type TemplateListResponse = {
  items: Array<{
    id: string;
    market: string;
    name: string;
    symbol: string;
  }>;
};

const defaultLevelsJson = JSON.stringify(
  [
    { entry_price: "97", quantity: "12.37113402", take_profit_bps: 220, trailing_bps: 70 },
    { entry_price: "98.5", quantity: "12.18274112", take_profit_bps: 220, trailing_bps: 70 },
    { entry_price: "101.5", quantity: "11.82266010", take_profit_bps: 220, trailing_bps: null },
    { entry_price: "103", quantity: "11.65048544", take_profit_bps: 220, trailing_bps: null },
  ],
  null,
  2,
);

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function StrategyNewPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const error = firstValue(params.error);
  const symbolQuery = firstValue(params.symbolQuery) ?? "";
  const [strategiesResult, templatesResult, symbolMatchesResult] = await Promise.all([
    fetchStrategies(),
    fetchTemplates(),
    fetchSymbolMatches(symbolQuery),
  ]);
  const strategies = strategiesResult.items;
  const templates = templatesResult.items;
  const symbolMatches = symbolMatchesResult.items;
  const loadError = strategiesResult.error ?? templatesResult.error ?? symbolMatchesResult.error;

  return (
    <>
      <StatusBanner
        description="Draft creation now captures amount mode, batch grid controls, symbol search, and the pause-before-edit lifecycle rules."
        title="Strategy creation workspace"
        tone="info"
      />
      {error ? <StatusBanner description={error} title="Draft creation failed" tone="warning" /> : null}
      {loadError ? <StatusBanner description={loadError} title="Strategy setup unavailable" tone="warning" /> : null}
      <AppShellSection
        description="Create a draft first, then save edits, run pre-flight, and start from the strategy workspace."
        eyebrow="Strategy creation"
        title="New Strategy"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Draft setup</CardTitle>
              <CardDescription>Choose batch controls for fast setup or switch to custom JSON for fully manual ladders.</CardDescription>
            </CardHeader>
            <CardBody>
              <form action="/app/strategies/new" id="symbol-search-form" method="get" />
              <FormStack action="/api/user/strategies/create" method="post">
                <Field label="Strategy name">
                  <Input defaultValue="ETH Swing Builder" name="name" required />
                </Field>
                <Field label="Search symbols" hint="Symbol search uses the synced Binance metadata for fuzzy matching.">
                  <div className="button-row">
                    <Input defaultValue={symbolQuery || "ETH"} form="symbol-search-form" name="symbolQuery" />
                    <Button form="symbol-search-form" type="submit">Search symbols</Button>
                  </div>
                </Field>
                <Field label="Symbol">
                  <Input defaultValue={symbolMatches[0]?.symbol ?? "ETHUSDT"} list="symbol-suggestions" name="symbol" required />
                  <datalist id="symbol-suggestions">
                    {symbolMatches.map((item) => (
                      <option key={item.symbol} value={item.symbol}>{item.market} · {item.base_asset}/{item.quote_asset}</option>
                    ))}
                  </datalist>
                </Field>
                <Field label="Market type">
                  <Select defaultValue="spot" name="marketType">
                    <option value="spot">Spot</option>
                    <option value="usd-m">USDⓈ-M futures</option>
                    <option value="coin-m">COIN-M futures</option>
                  </Select>
                </Field>
                <Field label="Strategy mode">
                  <Select defaultValue="classic" name="mode">
                    <option value="classic">Classic two-way spot</option>
                    <option value="buy-only">Buy-only spot grid</option>
                    <option value="sell-only">Sell-only spot grid</option>
                    <option value="long">Long futures grid</option>
                    <option value="short">Short futures grid</option>
                    <option value="neutral">Neutral futures grid</option>
                  </Select>
                </Field>
                <Field label="Generation mode">
                  <Select defaultValue="arithmetic" name="generation">
                    <option value="arithmetic">Arithmetic</option>
                    <option value="geometric">Geometric</option>
                    <option value="custom">Fully custom</option>
                  </Select>
                </Field>
                <Field label="Editor mode" hint="Batch mode quickly builds ladders. Custom JSON remains available for every-grid editing.">
                  <Select defaultValue="batch" name="editorMode">
                    <option value="batch">Batch ladder builder</option>
                    <option value="custom">Custom JSON</option>
                  </Select>
                </Field>
                <Field label="Amount mode" hint="Pick quote amount for USDT-sized grids or base quantity for coin-sized grids.">
                  <Select defaultValue="quote" name="amountMode">
                    <option value="quote">Quote amount</option>
                    <option value="base">Base asset quantity</option>
                  </Select>
                </Field>
                <Field label="Futures margin mode" hint="Required for futures strategies. Spot strategies ignore this setting.">
                  <Select defaultValue="isolated" name="futuresMarginMode">
                    <option value="isolated">Isolated</option>
                    <option value="cross">Cross</option>
                  </Select>
                </Field>
                <Field label="Leverage">
                  <Input defaultValue="5" inputMode="numeric" name="leverage" />
                </Field>
                <Field label="Quote amount (USDT)">
                  <Input defaultValue="1200" inputMode="decimal" name="quoteAmount" />
                </Field>
                <Field label="Base asset quantity">
                  <Input defaultValue="0.0100" inputMode="decimal" name="baseQuantity" />
                </Field>
                <Field label="Reference price">
                  <Input defaultValue="100" inputMode="decimal" name="referencePrice" />
                </Field>
                <Field label="Grid count">
                  <Input defaultValue="4" inputMode="numeric" name="gridCount" />
                </Field>
                <Field label="Batch spacing (%)">
                  <Input defaultValue="1.5" inputMode="decimal" name="gridSpacingPercent" />
                </Field>
                <Field label="Batch take profit (%)">
                  <Input defaultValue="2.2" inputMode="decimal" name="batchTakeProfit" />
                </Field>
                <Field label="Trailing take profit (%)">
                  <Input defaultValue="0.7" inputMode="decimal" name="batchTrailing" />
                </Field>
                <Field label="Overall take profit (%)">
                  <Input defaultValue="7.0" inputMode="decimal" name="overallTakeProfit" required />
                </Field>
                <Field label="Overall stop loss (%)">
                  <Input defaultValue="2.5" inputMode="decimal" name="overallStopLoss" />
                </Field>
                <Field label="Grid levels JSON" hint="Use this for fully custom ladders, per-grid overrides, or manual trailing rules.">
                  <textarea className="ui-input" defaultValue={defaultLevelsJson} name="levels_json" rows={10} />
                </Field>
                <Field label="Post-trigger behavior">
                  <Select defaultValue="rebuild" name="postTrigger">
                    <option value="stop">Stop after execution</option>
                    <option value="rebuild">Rebuild and continue</option>
                  </Select>
                </Field>
                <Button type="submit">Save draft</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Strategy templates</CardTitle>
              <CardDescription>Apply template presets into a user-owned draft, then customize freely.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/strategies/templates" method="post">
                <Field label="Strategy name">
                  <Input defaultValue="Template Based Draft" name="name" required />
                </Field>
                <Field label="Apply template">
                  <Select defaultValue={templates[0]?.id ?? ""} name="templateId">
                    {templates.length === 0 ? <option value="">No templates available</option> : null}
                    {templates.map((template) => (
                      <option key={template.id} value={template.id}>{template.name} · {template.symbol} · {template.market}</option>
                    ))}
                  </Select>
                </Field>
                <Button type="submit">Apply template</Button>
              </FormStack>
              <ul className="text-list">
                <li>Batch mode writes real `levels_json` before the API request is sent.</li>
                <li>Switch to Custom JSON when every grid needs manual entry price, quantity, TP, or trailing overrides.</li>
                <li>Futures allow only one strategy per user per symbol per direction.</li>
                <li>Existing user drafts: {strategies.length}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <DialogFrame
        description="Starting is blocked until pre-flight confirms exchange filters, balance, and the required hedge-mode posture. Trailing take profit uses taker execution and may increase fees."
        title="Pre-flight remains mandatory"
        tone="warning"
      />
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
    return { items: [], error: "Unable to load your current strategies." };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

async function fetchTemplates(): Promise<{ items: TemplateListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { items: [], error: null };
  }
  const response = await fetch(`${authApiBaseUrl()}/strategies/templates`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: [], error: "Unable to load strategy templates." };
  }
  return { items: ((await response.json()) as TemplateListResponse).items, error: null };
}

async function fetchSymbolMatches(query: string): Promise<{ items: SymbolSearchResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken || !query.trim()) {
    return { items: [], error: null };
  }
  const response = await fetch(`${authApiBaseUrl()}/exchange/binance/symbols/search`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ query }),
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: [], error: "Unable to search symbols right now." };
  }
  return { items: ((await response.json()) as SymbolSearchResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
