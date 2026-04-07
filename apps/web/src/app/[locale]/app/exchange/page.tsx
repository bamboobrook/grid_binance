import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type ExchangePageProps = {
  searchParams?: Promise<{
    error?: string | string[];
    exchange?: string | string[];
  }>;
};

type ExchangeAccountResponse = {
  account: {
    api_key_masked: string;
    connection_status: string;
    selected_markets: string[];
    validation: {
      can_read_coinm: boolean;
      can_read_spot: boolean;
      can_read_usdm: boolean;
      hedge_mode_ok: boolean;
      market_access_ok: boolean;
      permissions_ok: boolean;
    };
  };
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function ExchangePage({ searchParams }: ExchangePageProps) {
  const params = (await searchParams) ?? {};
  const notice = firstValue(params.exchange);
  const error = firstValue(params.error);
  const account = await fetchExchangeAccount();
  const positionMode = account ? (account.account.validation.hedge_mode_ok ? "hedge" : "one-way") : "hedge";
  const supportedScopes = account?.account.selected_markets?.length
    ? account.account.selected_markets.map(labelForMarket)
    : ["Spot", "USDⓈ-M", "COIN-M"];
  const state = describeConnectionState(account);

  return (
    <>
      <StatusBanner
        description="One user can bind only one Binance account, and saved API secrets stay encrypted and masked."
        title="Exchange credential workspace"
        tone="info"
      />
      {error ? <StatusBanner description={error} title="Exchange action failed" tone="warning" /> : null}
      {notice === "credentials-saved" ? (
        <StatusBanner
          description="The key is masked immediately after persistence and withdrawal permission must remain disabled."
          title="Credentials saved"
          tone="success"
        />
      ) : null}
      {notice === "test-passed" ? (
        <StatusBanner
          description="Spot, USDⓈ-M, and COIN-M permissions verified. Hedge mode remains required before futures pre-flight can pass."
          title="Connection test passed"
          tone="success"
        />
      ) : null}
      {notice === "test-failed" ? (
        <StatusBanner
          description="The saved Binance account is reachable, but the latest validation snapshot is not healthy enough for futures pre-flight."
          title="Connection test failed"
          tone="warning"
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
                  <Select defaultValue={positionMode} name="positionMode">
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
                <li>Masked API key: {account?.account.api_key_masked ?? "Not saved yet"}</li>
                <li>API secret: {account ? "••••••••••••••••" : "Not saved yet"}</li>
                <li>Connection status: {state.label}</li>
                <li>Supported scopes: {supportedScopes.join(", ")}</li>
                <li>Validation posture: {state.detail}</li>
                <li>Symbol metadata sync: Every 1 hour</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Validation details</CardTitle>
          <CardDescription>Connection testing now shows which exchange checks passed and which one blocks futures starts.</CardDescription>
        </CardHeader>
        <CardBody>
          <ul className="text-list">
            <li>Spot readable: {describeValidationValue(account, account?.account.validation.can_read_spot)}</li>
            <li>USDⓈ-M readable: {describeValidationValue(account, account?.account.validation.can_read_usdm)}</li>
            <li>COIN-M readable: {describeValidationValue(account, account?.account.validation.can_read_coinm)}</li>
            <li>Permissions OK: {describeValidationValue(account, account?.account.validation.permissions_ok)}</li>
            <li>Market access OK: {describeValidationValue(account, account?.account.validation.market_access_ok)}</li>
            <li>Hedge mode OK: {describeValidationValue(account, account?.account.validation.hedge_mode_ok)}</li>
          </ul>
        </CardBody>
      </Card>
      <DialogFrame
        description="If hedge mode, balance, or exchange filters do not match runtime requirements, strategy pre-flight must fail fast with the exact reason."
        title="Trading-critical warning"
        tone="warning"
      />
    </>
  );
}

function describeConnectionState(account: ExchangeAccountResponse | null) {
  if (!account?.account.api_key_masked) {
    return {
      label: "Not connected yet",
      detail: "未绑定 / Not connected yet",
    };
  }
  if (account.account.connection_status === "healthy") {
    return {
      label: "Verified",
      detail: "Permissions verified",
    };
  }
  if (account.account.connection_status === "pending" || account.account.connection_status === "untested") {
    return {
      label: "Awaiting validation",
      detail: "未测试 / Awaiting validation",
    };
  }
  return {
    label: "Validation failed",
    detail: "校验失败 / Validation failed",
  };
}

function describeValidationValue(account: ExchangeAccountResponse | null, value?: boolean) {
  if (!account?.account.api_key_masked) {
    return "Not configured";
  }
  return value ? "Yes" : "No";
}

async function fetchExchangeAccount(): Promise<ExchangeAccountResponse | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }

  const response = await fetch(authApiBaseUrl() + "/exchange/binance/account", {
    method: "GET",
    headers: {
      authorization: "Bearer " + sessionToken,
    },
    cache: "no-store",
  });

  if (!response.ok) {
    return null;
  }

  return (await response.json()) as ExchangeAccountResponse;
}

function labelForMarket(value: string) {
  switch (value) {
    case "spot":
      return "Spot";
    case "usdm":
      return "USDⓈ-M";
    case "coinm":
      return "COIN-M";
    default:
      return value;
  }
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
