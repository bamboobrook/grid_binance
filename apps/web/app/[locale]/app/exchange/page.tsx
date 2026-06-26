import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
  const keyState = (summarySnapshot?.api_key_masked || (hasSavedCredentialState ? pickText(lang, "已保存", "Saved") : "")) || pickText(lang, "未填写", "Not added");
  const checkItems = [
    { label: pickText(lang, "能连接币安", "Can reach Binance"), value: validationSnapshot?.validation.api_connectivity_ok },
    { label: pickText(lang, "API 权限正确", "API permissions are correct"), value: validationSnapshot?.validation.permissions_ok },
    { label: pickText(lang, "提现权限关闭", "Withdrawals are disabled"), value: validationSnapshot?.validation.withdrawals_disabled },
    { label: pickText(lang, "市场范围可用", "Selected markets are usable"), value: validationSnapshot?.validation.market_access_ok },
    { label: pickText(lang, "合约对冲模式", "Futures hedge mode"), value: validationSnapshot?.validation.hedge_mode_ok },
  ];

  return (
    <>
      {error ? <StatusBanner description={error} title={pickText(lang, "交易所操作失败", "Exchange action failed")}  tone="info" lang={lang} /> : null}
      {testResult ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={buildTestResultDescription(lang, testResult)}
          title={pickText(lang, "当前测试结果", "Current test result")}
        />
      ) : null}
      {notice === "credentials-saved" ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "密钥保存后会立刻变成掩码显示，并且提现权限必须保持关闭。", "The key is masked immediately after persistence and withdrawal permission must remain disabled.")}
          title={pickText(lang, "凭证已保存", "Credentials saved")}
        />
      ) : null}
      {!testResult && notice === "test-passed-saved" ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "当前输入已通过校验并自动保存，无需再点击保存凭证。", "The current input passed validation and was auto-saved, so no second save click is required.")}
          title={pickText(lang, "连接测试通过并已自动保存", "Connection test passed and auto-saved")}
        />
      ) : null}
      {!testResult && notice === "test-passed" ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "当前勾选的市场范围已通过校验；如需运行合约，仍必须保持对冲模式。", "The selected market scope passed validation. Futures still require hedge mode before strategy pre-flight can pass.")}
          title={pickText(lang, "连接测试通过", "Connection test passed")}
        />
      ) : null}
      {!testResult && notice === "test-failed" ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "请按下面的检查结果修改后，再测试一次。", "Review the checklist below, adjust the settings, then test again.")}
          title={pickText(lang, "连接没有通过", "Connection test failed")}
        />
      ) : null}
      <AppShellSection
        eyebrow={pickText(lang, "交易所", "Exchange")}
        title={pickText(lang, "连接币安", "Connect Binance")}
      >
        <div className="grid gap-3 md:grid-cols-3">
          <StatusTile label={pickText(lang, "连接状态", "Connection")} value={state.label} />
          <StatusTile label={pickText(lang, "API Key", "API key")} value={keyState} />
          <StatusTile label={pickText(lang, "可用市场", "Markets")} value={supportedScopes.join(" / ")} />
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_24rem]">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "填写 API", "Add API")}</CardTitle>
              <CardDescription>{pickText(lang, "在币安创建 API 后，把 Key 和 Secret 填到这里。先测试连接，通过后就可以创建机器人。", "Create an API on Binance, then paste the Key and Secret here. Test the connection before creating a bot.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/exchange" method="post" className="gap-4">
                <Field hint={pickText(lang, "不要开启提现权限", "No withdrawal permission")} label={pickText(lang, "币安 API Key", "Binance API key")}>
                  <Input name="apiKey" />
                </Field>
                <Field hint={pickText(lang, "保存后不会再显示明文", "Hidden after save")} label={pickText(lang, "币安 API Secret", "Binance API secret")}>
                  <Input name="apiSecret" type="password" />
                </Field>
                <Field hint={pickText(lang, "只选你要运行的市场", "Select only what you will trade")} label={pickText(lang, "交易市场", "Markets")}>
                  <div className="grid gap-2 sm:grid-cols-3">
                    <label className="flex items-center gap-2 rounded-md border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("spot")} name="selectedMarkets" type="checkbox" value="spot" />
                      <span>{pickText(lang, "现货", "Spot")}</span>
                    </label>
                    <label className="flex items-center gap-2 rounded-md border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("usdm")} name="selectedMarkets" type="checkbox" value="usdm" />
                      <span>{pickText(lang, "U本位合约", "USDⓈ-M futures")}</span>
                    </label>
                    <label className="flex items-center gap-2 rounded-md border border-border/70 px-3 py-2 text-sm text-foreground">
                      <input defaultChecked={selectedMarkets.includes("coinm")} name="selectedMarkets" type="checkbox" value="coinm" />
                      <span>{pickText(lang, "币本位合约", "COIN-M futures")}</span>
                    </label>
                  </div>
                </Field>
                <Field hint={pickText(lang, "仅现货网格可选单向模式", "One-way mode is only available for spot grids")} label={pickText(lang, "持仓模式", "Position mode")}>
                  <Select defaultValue={positionMode} name="positionMode">
                    <option value="hedge">{pickText(lang, "对冲模式", "Hedge mode")}</option>
                    <option value="one-way">{pickText(lang, "单向模式", "One-way")}</option>
                  </Select>
                </Field>
                <ButtonRow className="flex-wrap">
                  <Button name="intent" type="submit" value="save">
                    {pickText(lang, "保存 API", "Save API")}
                  </Button>
                  <Button name="intent" type="submit" value="test">
                    {pickText(lang, "测试连接", "Test connection")}
                  </Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "连接检查", "Connection checklist")}</CardTitle>
              <CardDescription>{pickText(lang, "点“测试连接”后，看这里有没有没通过的项目。", "After testing, check here for anything that did not pass.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="grid gap-2">
                {checkItems.map((item) => (
                  <CheckRow key={item.label} label={item.label} value={describeCheckState(lang, validationSnapshot, item.value)} />
                ))}
              </div>
              <div className="mt-4 rounded-md border border-border bg-secondary/40 p-3 text-xs leading-5 text-muted-foreground">
                {state.detail}
              </div>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}

function StatusTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-card p-4">
      <p className="text-xs font-bold uppercase text-muted-foreground">{label}</p>
      <p className="mt-2 truncate text-sm font-bold text-foreground">{value}</p>
    </div>
  );
}

function CheckRow({ label, value }: { label: string; value: { label: string; tone: "default" | "success" | "danger" } }) {
  const toneClass =
    value.tone === "success"
      ? "bg-emerald-500/10 text-emerald-500"
      : value.tone === "danger"
        ? "bg-red-500/10 text-red-500"
        : "bg-secondary text-muted-foreground";
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-border bg-background p-3">
      <span className="text-sm font-semibold text-foreground">{label}</span>
      <span className={`shrink-0 rounded-md px-2 py-1 text-xs font-bold ${toneClass}`}>{value.label}</span>
    </div>
  );
}

function describeConnectionState(lang: UiLanguage, snapshot: ExchangeAccountSnapshot | null) {
  if (!snapshot || snapshot.binding_state === "missing") {
    return {
      label: pickText(lang, "尚未连接", "Not connected yet"),
      detail: pickText(lang, "先填写 API，再点击测试连接。", "Add your API, then test the connection."),
    };
  }
  if (snapshot.binding_state === "partial") {
    return {
      label: pickText(lang, "已保存待恢复", "Saved, needs recovery"),
      detail: pickText(lang, "系统里有保存记录，但还需要重新测试一次。", "A saved record exists, but it needs another test."),
    };
  }
  if (snapshot.connection_status === "healthy") {
    return {
      label: pickText(lang, "已验证", "Verified"),
      detail: pickText(lang, "连接正常，可以继续创建机器人。", "Connection is ready. You can create a bot."),
    };
  }
  if (snapshot.connection_status === "pending" || snapshot.connection_status === "untested") {
    return {
      label: pickText(lang, "等待校验", "Awaiting validation"),
      detail: pickText(lang, "点击测试连接后，这里会显示结果。", "Test the connection to see the result here."),
    };
  }
  return {
    label: pickText(lang, "校验失败", "Validation failed"),
    detail: describeBlockingReason(lang, snapshot),
  };
}

function describeCheckState(lang: UiLanguage, snapshot: ExchangeAccountSnapshot | null, value?: boolean) {
  if (!snapshot || snapshot.binding_state === "missing") {
    return { label: pickText(lang, "未测试", "Not tested"), tone: "default" as const };
  }
  return value
    ? { label: pickText(lang, "通过", "Passed"), tone: "success" as const }
    : { label: pickText(lang, "未通过", "Failed"), tone: "danger" as const };
}

function buildTestResultDescription(lang: UiLanguage, result: ExchangeTestResult) {
  const synced = typeof result.synced_symbols === "number"
    ? pickText(lang, `已同步 ${result.synced_symbols} 个交易对。`, `Synced ${result.synced_symbols} symbols.`)
    : "";

  if (result.account.connection_status === "healthy") {
    return [
      result.persisted
        ? pickText(lang, "连接成功，API 已保存。", "Connection passed and the API is saved.")
        : pickText(lang, "连接成功，但保存失败，请重新保存一次。", "Connection passed, but saving failed. Save again."),
      synced,
    ].filter(Boolean).join(" ");
  }

  return [
    pickText(lang, `连接没有通过：${describeBlockingReason(lang, result.account)}。`, `Connection failed: ${describeBlockingReason(lang, result.account)}.`),
    buildScopeAdjustmentHint(lang, result.account),
    synced,
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
