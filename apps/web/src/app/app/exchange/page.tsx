import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

export default async function ExchangePage() {
  const state = await getCurrentUserProductState();

  return (
    <>
      <StatusBanner
        description="One user can bind only one Binance account, and saved API secrets stay encrypted and masked."
        title="Exchange credential workspace"
        tone="info"
      />
      {state.flash.exchange ? (
        <StatusBanner
          description={
            state.exchange.connectionStatus === "passed"
              ? "Spot, USDⓈ-M, and COIN-M permissions verified. Hedge mode remains required before futures pre-flight can pass."
              : state.flash.exchange === "Credentials saved"
                ? "The key is masked immediately after persistence and withdrawal permission must remain disabled."
                : state.flash.exchange
          }
          title={state.flash.exchange}
          tone={state.exchange.connectionStatus === "failed" ? "warning" : "success"}
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
              <CardDescription>Credentials are submitted over POST and never round-tripped through the URL.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/exchange" method="post">
                <Field hint="Do not enable withdrawal permission on your Binance API key." label="Binance API key">
                  <Input name="apiKey" />
                </Field>
                <Field hint="Stored encrypted server-side and never shown back in plaintext." label="Binance API secret">
                  <Input name="apiSecret" type="password" />
                </Field>
                <Field hint="Required for futures strategies in V1." label="Position mode">
                  <Select defaultValue={state.exchange.positionMode} name="positionMode">
                    <option value="hedge">Hedge mode</option>
                    <option value="one-way">One-way</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button name="intent" type="submit" value="save">
                    Save credentials
                  </Button>
                  <Button name="intent" tone="secondary" type="submit" value="test">
                    Run connection test
                  </Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Credential summary</CardTitle>
              <CardDescription>Masked values and exchange runtime requirements remain visible after save.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Masked API key: {state.exchange.apiKeyMasked ?? "Not saved yet"}</li>
                <li>API secret: {state.exchange.saved ? "••••••••••••••••" : "Not saved yet"}</li>
                <li>Connection status: {state.exchange.connectionStatus === "passed" ? "Verified" : state.exchange.saved ? "Saved, not tested" : "Not connected"}</li>
                <li>Supported scopes: {state.exchange.supportedScopes.join(", ")}</li>
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
