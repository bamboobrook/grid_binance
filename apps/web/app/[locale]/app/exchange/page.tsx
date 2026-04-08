import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DialogFrame } from "@/components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type ExchangePageProps = {
  params: Promise<{ locale: string }>;
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

export default async function ExchangePage({ params, searchParams }: ExchangePageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const query = (await searchParams) ?? {};
  const notice = firstValue(query.exchange);
  const error = firstValue(query.error);
  const account = await fetchExchangeAccount();
  const positionMode = account ? (account.account.validation.hedge_mode_ok ? "hedge" : "one-way") : "hedge";
  const supportedScopes = account?.account.selected_markets?.length
    ? account.account.selected_markets.map((value) => labelForMarket(lang, value))
    : [pickText(lang, "现货", "Spot"), "USDⓈ-M", "COIN-M"];
  const state = describeConnectionState(lang, account);

  return (
    <>
      <StatusBanner
        description={pickText(lang, "一个用户只能绑定一个币安账户，保存后的密钥会立即加密并掩码显示。", "One user can bind only one Binance account, and saved API secrets are immediately encrypted and masked.")}
        title={pickText(lang, "交易所凭证工作区", "Exchange credential workspace")}
      />
      {error ? <StatusBanner description={error} title={pickText(lang, "交易所操作失败", "Exchange action failed")} tone="danger" /> : null}
      {notice === "credentials-saved" ? (
        <StatusBanner
          description={pickText(lang, "密钥保存后会立刻掩码显示，且绝不能开启提现权限。", "The key is masked immediately after save, and withdrawal permission must remain disabled.")}
          title={pickText(lang, "凭证已保存", "Credentials saved")}
        />
      ) : null}
      {notice === "test-passed" ? (
        <StatusBanner
          description={pickText(lang, "现货、USDⓈ-M 与 COIN-M 权限均已验证；合约预检仍要求双向持仓模式。", "Spot, USDⓈ-M, and COIN-M permissions are verified. Futures pre-flight still requires hedge mode.")}
          title={pickText(lang, "连接测试通过", "Connection test passed")}
        />
      ) : null}
      {notice === "test-failed" ? (
        <StatusBanner
          description={pickText(lang, "账户可达，但最新校验快照仍不足以通过合约预检。", "The saved Binance account is reachable, but the latest validation snapshot is still not healthy enough for futures pre-flight.")}
          title={pickText(lang, "连接测试失败", "Connection test failed")}
          tone="warning"
        />
      ) : null}
      <AppShellSection
        description={pickText(lang, "策略启动前，这里会持续展示凭证保存、权限校验和持仓模式结果。", "Credential save, permission checks, and position-mode validation stay visible here before any strategy start.")}
        eyebrow={pickText(lang, "交易所设置", "Exchange settings")}
        title={pickText(lang, "交易所凭证", "Exchange Credentials")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "绑定币安账户", "Bind Binance account")}</CardTitle>
              <CardDescription>{pickText(lang, "凭证通过 POST 提交，不会在 URL 中回显。", "Credentials are submitted over POST and never round-tripped in the URL.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/exchange" method="post">
                <Field hint={pickText(lang, "请不要给 API 开启提现权限。", "Do not enable withdrawal permission on the Binance API key.")} label={pickText(lang, "币安 API Key", "Binance API key")}>
                  <Input name="apiKey" />
                </Field>
                <Field hint={pickText(lang, "服务器端加密保存，之后不会明文展示。", "Stored encrypted server-side and never shown back in plaintext.")} label={pickText(lang, "币安 API Secret", "Binance API secret")}>
                  <Input name="apiSecret" type="password" />
                </Field>
                <Field hint={pickText(lang, "首版合约策略要求双向持仓模式。", "Hedge mode is required for futures strategies in V1.")} label={pickText(lang, "持仓模式", "Position mode")}>
                  <Select defaultValue={positionMode} name="positionMode">
                    <option value="hedge">{pickText(lang, "双向持仓", "Hedge mode")}</option>
                    <option value="one-way">{pickText(lang, "单向持仓", "One-way")}</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button name="intent" type="submit" value="save">
                    {pickText(lang, "保存凭证", "Save credentials")}
                  </Button>
                  <Button name="intent" type="submit" value="test">
                    {pickText(lang, "执行连接测试", "Run connection test")}
                  </Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "凭证摘要", "Credential summary")}</CardTitle>
              <CardDescription>{pickText(lang, "保存后继续展示掩码值和交易所运行要求。", "Masked values and exchange runtime requirements remain visible after save.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "掩码 API Key", "Masked API key")}: {account?.account.api_key_masked ?? pickText(lang, "尚未保存", "Not saved yet")}</li>
                <li>{pickText(lang, "API Secret", "API secret")}: {account ? "••••••••••••••••" : pickText(lang, "尚未保存", "Not saved yet")}</li>
                <li>{pickText(lang, "连接状态", "Connection status")}: {state.label}</li>
                <li>{pickText(lang, "已选市场", "Supported scopes")}: {supportedScopes.join(", ")}</li>
                <li>{pickText(lang, "校验摘要", "Validation posture")}: {state.detail}</li>
                <li>{pickText(lang, "交易对元数据同步", "Symbol metadata sync")}: {pickText(lang, "每 1 小时", "Every 1 hour")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "校验明细", "Validation details")}</CardTitle>
          <CardDescription>{pickText(lang, "连接测试会明确指出哪一步通过、哪一步阻塞了合约启动。", "Connection testing explicitly shows which check passed and which one is blocking futures starts.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <ul className="text-list">
            <li>{pickText(lang, "可读取现货", "Spot readable")}: {describeValidationValue(lang, account, account?.account.validation.can_read_spot)}</li>
            <li>{pickText(lang, "可读取 USDⓈ-M", "USDⓈ-M readable")}: {describeValidationValue(lang, account, account?.account.validation.can_read_usdm)}</li>
            <li>{pickText(lang, "可读取 COIN-M", "COIN-M readable")}: {describeValidationValue(lang, account, account?.account.validation.can_read_coinm)}</li>
            <li>{pickText(lang, "权限校验", "Permissions OK")}: {describeValidationValue(lang, account, account?.account.validation.permissions_ok)}</li>
            <li>{pickText(lang, "市场访问校验", "Market access OK")}: {describeValidationValue(lang, account, account?.account.validation.market_access_ok)}</li>
            <li>{pickText(lang, "双向持仓校验", "Hedge mode OK")}: {describeValidationValue(lang, account, account?.account.validation.hedge_mode_ok)}</li>
          </ul>
        </CardBody>
      </Card>
      <DialogFrame
        description={pickText(lang, "只要双向持仓、余额或交易所过滤器与运行要求不匹配，策略预检就必须明确失败并说明原因。", "If hedge mode, balance, or exchange filters do not match runtime requirements, strategy pre-flight must fail fast with the exact reason.")}
        title={pickText(lang, "交易关键提醒", "Trading-critical warning")}
      />
    </>
  );
}

function describeConnectionState(lang: UiLanguage, account: ExchangeAccountResponse | null) {
  if (!account?.account.api_key_masked) {
    return {
      label: pickText(lang, "尚未连接", "Not connected yet"),
      detail: pickText(lang, "尚未绑定", "Not connected yet"),
    };
  }
  if (account.account.connection_status === "healthy") {
    return {
      label: pickText(lang, "已验证", "Verified"),
      detail: pickText(lang, "权限已验证", "Permissions verified"),
    };
  }
  if (account.account.connection_status === "pending" || account.account.connection_status === "untested") {
    return {
      label: pickText(lang, "等待校验", "Awaiting validation"),
      detail: pickText(lang, "尚未测试", "Awaiting validation"),
    };
  }
  return {
    label: pickText(lang, "校验失败", "Validation failed"),
    detail: pickText(lang, "存在阻塞项", "Validation failed"),
  };
}

function describeValidationValue(lang: UiLanguage, account: ExchangeAccountResponse | null, value?: boolean) {
  if (!account?.account.api_key_masked) {
    return pickText(lang, "未配置", "Not configured");
  }
  return value ? pickText(lang, "是", "Yes") : pickText(lang, "否", "No");
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

function labelForMarket(lang: UiLanguage, value: string) {
  switch (value) {
    case "spot":
      return pickText(lang, "现货", "Spot");
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
