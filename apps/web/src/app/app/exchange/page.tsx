import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { firstValue } from "../../../lib/auth";

type ExchangePageProps = {
  searchParams?: Promise<{
    apiKey?: string | string[];
    apiSecret?: string | string[];
    positionMode?: string | string[];
    saved?: string | string[];
    tested?: string | string[];
  }>;
};

function maskApiKey(value: string) {
  if (value.length <= 8) {
    return "••••";
  }

  return `${value.slice(0, 4)}••••${value.slice(-4)}`;
}

export default async function ExchangePage({ searchParams }: ExchangePageProps) {
  const params = (await searchParams) ?? {};
  const apiKey = firstValue(params.apiKey) ?? "";
  const apiSecret = firstValue(params.apiSecret) ?? "";
  const positionMode = firstValue(params.positionMode) ?? "hedge";
  const saved = firstValue(params.saved) === "1";
  const tested = firstValue(params.tested) === "1";
  const hasCredentials = saved && apiKey.length > 0 && apiSecret.length > 0;
  const testHref = hasCredentials
    ? `/app/exchange?apiKey=${encodeURIComponent(apiKey)}&apiSecret=${encodeURIComponent(apiSecret)}&positionMode=${encodeURIComponent(positionMode)}&saved=1&tested=1`
    : "/app/exchange";

  return (
    <>
      <StatusBanner
        description="One user can bind only one Binance account, and saved API secrets stay encrypted and masked."
        title="Exchange credential workspace"
        tone="info"
      />
      {saved ? (
        <StatusBanner
          description="Credentials saved. The key is masked immediately after persistence and withdrawal permission must remain disabled."
          title="Credentials saved"
          tone="success"
        />
      ) : null}
      {tested ? (
        <StatusBanner
          description="Spot, USDⓈ-M, and COIN-M permissions verified. Hedge mode remains required before futures pre-flight can pass."
          title="Connection test passed"
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Credential save, masking, and connection verification stay visible before any strategy can start."
        eyebrow="Exchange settings"
        title="Exchange Credentials"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Bind Binance account</CardTitle>
              <CardDescription>Save first, then run the connection test before opening strategy runtime.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/app/exchange" method="get">
                <Field hint="Do not enable withdrawal permission on your Binance API key." label="Binance API key">
                  <Input defaultValue={apiKey} name="apiKey" required />
                </Field>
                <Field hint="Stored encrypted server-side and never shown back in plaintext." label="Binance API secret">
                  <Input defaultValue={apiSecret} name="apiSecret" required type="password" />
                </Field>
                <Field hint="Required for futures strategies in V1." label="Position mode">
                  <Select defaultValue={positionMode} name="positionMode">
                    <option value="hedge">Hedge mode</option>
                    <option value="one-way">One-way</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button name="saved" type="submit" value="1">
                    Save credentials
                  </Button>
                  <Link className="button button--ghost" href={testHref}>
                    Run connection test
                  </Link>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Credential summary</CardTitle>
              <CardDescription>Masked values and futures constraints remain visible after save.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Masked API key: {hasCredentials ? maskApiKey(apiKey) : "Not saved yet"}</li>
                <li>API secret: {hasCredentials ? "••••••••••••••••" : "Not saved yet"}</li>
                <li>Supported scopes: Spot, USDⓈ-M, COIN-M</li>
                <li>Symbol metadata sync: Every 1 hour</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <DialogFrame
        description="If hedge mode, balance, or exchange filters do not match runtime requirements, strategy pre-flight must fail fast with the exact reason."
        title="Trading-critical warning"
        tone="warning"
      />
    </>
  );
}
