import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DialogFrame } from "@/components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const EXCHANGE_TEST_RESULT_COOKIE = "exchange_test_result";

type ExchangePageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    error?: string | string[];
    exchange?: string | string[];
  }>;
};

type ExchangeAccountSnapshot = {
  api_key_masked: string;
  binding_state: string;
  connection_status: string;
  sync_status: string;
  selected_markets: string[];
  validation: {
    api_connectivity_ok: boolean;
    timestamp_in_sync: boolean;
    can_read_coinm: boolean;
    can_read_spot: boolean;
    can_read_usdm: boolean;
    hedge_mode_ok: boolean;
    market_access_ok: boolean;
    permissions_ok: boolean;
    withdrawals_disabled: boolean;
  };
};

type ExchangeAccountResponse = {
  account: ExchangeAccountSnapshot;
};

type ExchangeTestResult = {
  account: ExchangeAccountSnapshot;
  synced_symbols?: number;
  persisted?: boolean;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function ExchangePage({ params, searchParams }: ExchangePageProps) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const query = (await searchParams) ?? {};
  const notice = firstValue(query.exchange);
  const error = firstValue(query.error);
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  const account = await fetchExchangeAccount(sessionToken);
  const testResult = readExchangeTestResult(cookieStore);
  const persistedSnapshot = account?.account ?? null;
  const validationSnapshot = testResult?.account ?? persistedSnapshot;
  const summarySnapshot = persistedSnapshot ?? validationSnapshot;
  const positionMode = validationSnapshot ? (validationSnapshot.validation.hedge_mode_ok ? "hedge" : "one-way") : "hedge";
  const configuredMarkets = validationSnapshot?.selected_markets ?? [];
  const selectedMarkets = configuredMarkets.length > 0 ? configuredMarkets : ["spot", "usdm", "coinm"];
  const summaryMarkets = summarySnapshot?.selected_markets ?? [];
  const hasSavedCredentialState = Boolean(summarySnapshot && summarySnapshot.binding_state !== "missing");
  const supportedScopes = hasSavedCredentialState
    ? (summaryMarkets.length > 0 ? summaryMarkets.map((value) => labelForMarket(lang, value)) : [pickText(lang, "待重新校验", "Needs re-validation")])
    : [pickText(lang, "未配置", "Not configured")];
  const state = describeConnectionState(lang, summarySnapshot);
  const summaryTitle = persistedSnapshot || !testResult
    ? pickText(lang, "凭证摘要", "Credential summary")
    : pickText(lang, "当前测试结果（未自动保存）", "Current test result (not auto-saved)");
  const summaryDescription = persistedSnapshot
    ? (testResult
        ? pickText(lang, "上方横幅显示本次测试结果，下面继续保留已保存的凭证摘要。", "The banner above shows the latest validation result while the saved credential summary stays visible below.")
        : pickText(lang, "保存后仍会持续展示掩码与运行要求。", "Masked values and runtime requirements remain visible after save."))
    : pickText(lang, "这里只展示当前未落库的即时校验结果；测试通过时系统会自动保存，只有失败或自动保存失败时才会停留在这里。", "This panel only shows validation results that were not persisted. Successful tests auto-save, so only failures or auto-save failures remain here.");

  return (
    <>
      <StatusBanner
        description={pickText(lang, "一个用户只能绑定一个币安账户，保存后的密钥会立即加密并掩码显示。", "One user can bind only one Binance account, and saved API secrets stay encrypted and masked.")}
        title={pickText(lang, "交易所凭证工作区", "Exchange credential workspace")}
      />
      {error ? <StatusBanner description={error} title={pickText(lang, "交易所操作失败", "Exchange action failed")} /> : null}
      {testResult ? (
        <StatusBanner
          description={buildTestResultDescription(lang, testResult)}
          title={pickText(lang, "当前测试结果", "Current test result")}
        />
      ) : null}
      {notice === "credentials-saved" ? (
        <StatusBanner
          description={pickText(lang, "密钥保存后会立刻变成掩码显示，并且提现权限必须保持关闭。", "The key is masked immediately after persistence and withdrawal permission must remain disabled.")}
          title={pickText(lang, "凭证已保存", "Credentials saved")}
        />
      ) : null}
      {!testResult && notice === "test-passed-saved" ? (
        <StatusBanner
          description={pickText(lang, "当前输入已通过校验并自动保存，无需再点击保存凭证。", "The current input passed validation and was auto-saved, so no second save click is required.")}
          title={pickText(lang, "连接测试通过并已自动保存", "Connection test passed and auto-saved")}
        />
      ) : null}
      {!testResult && notice === "test-passed" ? (
        <StatusBanner
          description={pickText(lang, "当前勾选的市场范围已通过校验；如需运行合约，仍必须保持对冲模式。", "The selected market scope passed validation. Futures still require hedge mode before strategy pre-flight can pass.")}
          title={pickText(lang, "连接测试通过", "Connection test passed")}
        />
      ) : null}
      {!testResult && notice === "test-failed" ? (
        <StatusBanner
          description={pickText(lang, "账户虽然可达，但你当前勾选的市场范围仍有未通过项。", "The Binance account is reachable, but the currently selected market scope still has failing checks.")}
          title={pickText(lang, "连接测试失败", "Connection test failed")}
        />
      ) : null}
      <AppShellSection
        description={pickText(lang, "保存、掩码、连接校验都会在启动策略前完整展示。", "Credential save, masking, and connection verification stay visible before any strategy can start.")}
        eyebrow={pickText(lang, "交易所设置", "Exchange settings")}
        title={pickText(lang, "交易所凭证", "Exchange Credentials")}
      >
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "绑定币安账户", "Bind Binance account")}</CardTitle>
              <CardDescription>{pickText(lang, "凭证通过 POST 提交，不会通过 URL 回显；运行连接测试在通过时会自动保存当前输入。", "Credentials are submitted over POST, never round-tripped through the URL, and successful tests auto-save the current input.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/exchange" method="post" className="gap-4">
                <Field hint={pickText(lang, "请不要给币安 API 开启提现权限。", "Do not enable withdrawal permission on your Binance API key.")} label={pickText(lang, "币安 API Key", "Binance API key")}>
                  <Input name="apiKey" />
                </Field>
                <Field hint={pickText(lang, "只会加密保存在服务端，不会明文回显。", "Stored encrypted server-side and never shown back in plaintext.")} label={pickText(lang, "币安 API Secret", "Binance API secret")}>
                  <Input name="apiSecret" type="password" />
                </Field>
                <Field hint={pickText(lang, "只勾选你准备实际运行的市场；如果密钥只开了合约，请不要勾选现货。", "Only select the markets you actually plan to run. If this key is futures-only, leave Spot unchecked.")} label={pickText(lang, "选择测试市场", "Choose market scope")}>
                  <div className="grid gap-2 sm:grid-cols-3">
                    <label className="flex items-center gap-2 rounded-full border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("spot")} name="selectedMarkets" type="checkbox" value="spot" />
                      <span>{pickText(lang, "现货", "Spot")}</span>
                    </label>
                    <label className="flex items-center gap-2 rounded-full border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("usdm")} name="selectedMarkets" type="checkbox" value="usdm" />
                      <span>{pickText(lang, "U本位合约", "USDⓈ-M futures")}</span>
                    </label>
                    <label className="flex items-center gap-2 rounded-full border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("coinm")} name="selectedMarkets" type="checkbox" value="coinm" />
                      <span>{pickText(lang, "币本位合约", "COIN-M futures")}</span>
                    </label>
                  </div>
                </Field>
                <Field hint={pickText(lang, "如需运行合约策略，必须使用对冲模式。", "Required for futures strategies in V1.")} label={pickText(lang, "持仓模式", "Position mode")}>
                  <Select defaultValue={positionMode} name="positionMode">
                    <option value="hedge">{pickText(lang, "对冲模式", "Hedge mode")}</option>
                    <option value="one-way">{pickText(lang, "单向模式", "One-way")}</option>
                  </Select>
                </Field>
                <ButtonRow className="flex-wrap">
                  <Button name="intent" type="submit" value="save">
                    {pickText(lang, "保存凭证", "Save credentials")}
                  </Button>
                  <Button name="intent" type="submit" value="test">
                    {pickText(lang, "运行连接测试", "Run connection test")}
                  </Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{summaryTitle}</CardTitle>
              <CardDescription>{summaryDescription}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "掩码 API Key", "Masked API key")}: {(summarySnapshot?.api_key_masked || (hasSavedCredentialState ? pickText(lang, "已保存，待恢复摘要", "Saved, summary pending") : "")) || pickText(lang, "尚未保存", "Not saved yet")}</li>
                <li>{pickText(lang, "API Secret", "API secret")}: {hasSavedCredentialState ? "••••••••••••••••" : pickText(lang, "尚未保存", "Not saved yet")}</li>
                <li>{pickText(lang, "连接状态", "Connection status")}: {state.label}</li>
                <li>{pickText(lang, "支持范围", "Supported scopes")}: {supportedScopes.join(", ")}</li>
                <li>{pickText(lang, "校验结果", "Validation posture")}: {state.detail}</li>
                <li>{pickText(lang, "交易对同步", "Symbol metadata sync")}: {pickText(lang, "每1小时一次", "Every 1 hour")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "校验详情", "Validation details")}</CardTitle>
          <CardDescription>{pickText(lang, "连接测试会告诉你具体是哪一步阻塞了合约启动。", "Connection testing shows which exact exchange checks passed and which one blocks futures starts.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <ul className="text-list">
            <li>{pickText(lang, "API 连通性", "API connectivity")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.api_connectivity_ok)}</li>
            <li>{pickText(lang, "时间戳同步", "Timestamp sync")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.timestamp_in_sync)}</li>
            <li>{pickText(lang, "现货可读", "Spot readable")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.can_read_spot)}</li>
            <li>{pickText(lang, "U本位可读", "USDⓈ-M readable")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.can_read_usdm)}</li>
            <li>{pickText(lang, "币本位可读", "COIN-M readable")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.can_read_coinm)}</li>
            <li>{pickText(lang, "权限通过", "Permissions OK")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.permissions_ok)}</li>
            <li>{pickText(lang, "提现权限已关闭", "Withdrawals disabled")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.withdrawals_disabled)}</li>
            <li>{pickText(lang, "市场访问通过", "Market access OK")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.market_access_ok)}</li>
            <li>{pickText(lang, "对冲模式通过", "Hedge mode OK")}: {describeValidationValue(lang, validationSnapshot, validationSnapshot?.validation.hedge_mode_ok)}</li>
          </ul>
        </CardBody>
      </Card>
      <DialogFrame
        description={pickText(lang, "如果对冲模式、余额或交易所过滤器不满足要求，策略预检必须明确报出失败原因。", "If hedge mode, balance, or exchange filters do not match runtime requirements, strategy pre-flight must fail fast with the exact reason.")}
        lang={lang}
        title={pickText(lang, "交易级风险提醒", "Trading-critical warning")}
      />
    </>
  );
}

function describeConnectionState(lang: UiLanguage, snapshot: ExchangeAccountSnapshot | null) {
  if (!snapshot || snapshot.binding_state === "missing") {
    return {
      label: pickText(lang, "尚未连接", "Not connected yet"),
      detail: pickText(lang, "尚未绑定", "Not connected yet"),
    };
  }
  if (snapshot.binding_state === "partial") {
    return {
      label: pickText(lang, "已保存待恢复", "Saved, needs recovery"),
      detail: pickText(lang, "凭证仍在系统里，但摘要记录不完整，请重新运行一次连接测试。", "The credentials are still saved, but the summary record is incomplete. Run the connection test again."),
    };
  }
  if (snapshot.connection_status === "healthy") {
    return {
      label: pickText(lang, "已验证", "Verified"),
      detail: pickText(lang, "权限已通过校验", "Permissions verified"),
    };
  }
  if (snapshot.connection_status === "pending" || snapshot.connection_status === "untested") {
    return {
      label: pickText(lang, "等待校验", "Awaiting validation"),
      detail: pickText(lang, "尚未完成测试", "Awaiting validation"),
    };
  }
  return {
    label: pickText(lang, "校验失败", "Validation failed"),
    detail: describeBlockingReason(lang, snapshot),
  };
}

function describeValidationValue(lang: UiLanguage, snapshot: ExchangeAccountSnapshot | null, value?: boolean) {
  if (!snapshot || snapshot.binding_state === "missing") {
    return pickText(lang, "未配置", "Not configured");
  }
  return value ? pickText(lang, "是", "Yes") : pickText(lang, "否", "No");
}

function buildTestResultDescription(lang: UiLanguage, result: ExchangeTestResult) {
  const synced = typeof result.synced_symbols === "number"
    ? pickText(lang, `本次共校验并同步了 ${result.synced_symbols} 个交易对元数据。`, `Validated and synced ${result.synced_symbols} symbol metadata records during this test.`)
    : "";

  if (result.account.connection_status === "healthy") {
    return [
      result.persisted
        ? pickText(lang, "当前输入已通过完整校验，并已自动保存到系统。", "The current input passed the full validation flow and has been auto-saved.")
        : pickText(lang, "当前输入已通过完整校验，但自动保存失败，请先处理上方错误后重新提交。", "The current input passed the full validation flow, but auto-save failed. Resolve the error above and submit again."),
      synced,
    ].filter(Boolean).join(" ");
  }

  return [
    pickText(lang, `当前输入未通过校验，阻塞步骤：${describeBlockingReason(lang, result.account)}。`, `The current input failed validation. Blocking checks: ${describeBlockingReason(lang, result.account)}.`),
    buildScopeAdjustmentHint(lang, result.account),
    synced,
    pickText(lang, "由于测试未通过，本次结果不会保存到系统。", "The result was not persisted because the validation did not pass."),
  ].filter(Boolean).join(" ");
}

function buildScopeAdjustmentHint(lang: UiLanguage, snapshot: ExchangeAccountSnapshot) {
  const futuresAccessible = snapshot.validation.can_read_usdm || snapshot.validation.can_read_coinm;
  if (snapshot.selected_markets.includes("spot") && !snapshot.validation.can_read_spot && futuresAccessible) {
    return pickText(lang, "如果你只运行合约，请取消现货范围后重新测试。", "If you only run futures, clear Spot from the scope and test again.");
  }
  return "";
}

function describeBlockingReason(lang: UiLanguage, snapshot: ExchangeAccountSnapshot) {
  const failures: string[] = [];
  if (!snapshot.validation.api_connectivity_ok) {
    failures.push(pickText(lang, "API 连通性失败", "API connectivity failed"));
  }
  if (!snapshot.validation.timestamp_in_sync) {
    failures.push(pickText(lang, "服务器时间偏差过大", "Server time drift is too large"));
  }
  if (snapshot.selected_markets.includes("spot") && !snapshot.validation.can_read_spot) {
    failures.push(pickText(lang, "现货账户不可读", "Spot account is not readable"));
  }
  if (snapshot.selected_markets.includes("usdm") && !snapshot.validation.can_read_usdm) {
    failures.push(pickText(lang, "U本位账户不可读", "USDⓈ-M account is not readable"));
  }
  if (snapshot.selected_markets.includes("coinm") && !snapshot.validation.can_read_coinm) {
    failures.push(pickText(lang, "币本位账户不可读", "COIN-M account is not readable"));
  }
  if (!snapshot.validation.permissions_ok) {
    failures.push(pickText(lang, "API 权限不足", "API permissions are insufficient"));
  }
  if (!snapshot.validation.withdrawals_disabled) {
    failures.push(pickText(lang, "提现权限未关闭", "Withdrawal permission is still enabled"));
  }
  if (!snapshot.validation.market_access_ok) {
    failures.push(pickText(lang, "市场访问范围不完整", "Market access is incomplete"));
  }
  if (snapshot.selected_markets.some((market) => market === "usdm" || market === "coinm") && !snapshot.validation.hedge_mode_ok) {
    failures.push(pickText(lang, "对冲模式未开启", "Hedge mode is not enabled"));
  }
  return failures.length ? failures.join(" / ") : pickText(lang, "全部检查通过", "All checks passed");
}

async function fetchExchangeAccount(sessionToken: string): Promise<ExchangeAccountResponse | null> {
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

function readExchangeTestResult(cookieStore: Awaited<ReturnType<typeof cookies>>) {
  const raw = cookieStore.get(EXCHANGE_TEST_RESULT_COOKIE)?.value;
  if (!raw) {
    return null;
  }

  try {
    return JSON.parse(decodeURIComponent(raw)) as ExchangeTestResult;
  } catch {
    return null;
  }
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
