import { cookies } from "next/headers";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage, type UiLanguage } from "../../../../lib/ui/preferences";

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
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const params = (await searchParams) ?? {};
  const error = firstValue(params.error);
  const symbolQuery = firstValue(params.symbolQuery) ?? "";
  const results = await Promise.all([fetchStrategies(), fetchTemplates(), fetchSymbolMatches(symbolQuery)]);
  const strategiesResult = results[0];
  const templatesResult = results[1];
  const symbolMatchesResult = results[2];
  const strategies = strategiesResult.items;
  const templates = templatesResult.items;
  const symbolMatches = symbolMatchesResult.items;
  const loadError = strategiesResult.error ?? templatesResult.error ?? symbolMatchesResult.error;

  return (
    <>
      <StatusBanner
        description={pickText(lang, "新建页优先保证草稿可落地、预检前置信息明确、模板可直接复用。", "This page prioritizes draft readiness, clear pre-flight prerequisites, and template reuse.")}
        title={pickText(lang, "新建策略状态条", "New strategy status strip")}
        tone="info"
      />
      {error ? <StatusBanner description={error} title={pickText(lang, "草稿创建失败", "Draft creation failed")} tone="warning" /> : null}
      {loadError ? <StatusBanner description={loadError} title={pickText(lang, "策略创建上下文不可用", "Strategy setup unavailable")} tone="warning" /> : null}
      <AppShellSection
        description={pickText(lang, "主面板填写交易参数，侧栏只保留模板、容量与风控提醒。", "The main panel owns trade parameters while the side rail keeps templates, capacity, and risk reminders." )}
        eyebrow={pickText(lang, "新建策略", "Strategy creation")}
        title={pickText(lang, "新建策略", "New Strategy")}
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "草稿主面板", "Draft setup")}</CardTitle>
              <CardDescription>{pickText(lang, "先确定标的、模式和网格生成方式，再保存草稿。", "Choose symbol, mode, and ladder generation first, then save the draft.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <form action="/app/strategies/new" id="symbol-search-form" method="get" />
              <FormStack action="/api/user/strategies/create" method="post">
                <Field label={pickText(lang, "策略名", "Strategy name")}>
                  <Input defaultValue="ETH Swing Builder" name="name" required />
                </Field>
                <Field label={pickText(lang, "搜索交易对", "Search symbols")} hint={pickText(lang, "符号搜索使用已同步的 Binance 元数据。", "Symbol search uses synced Binance metadata.")}>
                  <div className="button-row">
                    <Input defaultValue={symbolQuery || "ETH"} form="symbol-search-form" name="symbolQuery" />
                    <Button form="symbol-search-form" type="submit">{pickText(lang, "搜索", "Search")}</Button>
                  </div>
                </Field>
                <Field label={pickText(lang, "交易对", "Symbol")}>
                  <Input defaultValue={symbolMatches[0]?.symbol ?? "ETHUSDT"} list="symbol-suggestions" name="symbol" required />
                  <datalist id="symbol-suggestions">
                    {symbolMatches.map((item) => (
                      <option key={item.symbol} value={item.symbol}>{item.market + " · " + item.base_asset + "/" + item.quote_asset}</option>
                    ))}
                  </datalist>
                </Field>
                <Field label={pickText(lang, "市场类型", "Market type")}>
                  <Select defaultValue="spot" name="marketType">
                    <option value="spot">{pickText(lang, "现货", "Spot")}</option>
                    <option value="usd-m">{pickText(lang, "U 本位合约", "USD-M futures")}</option>
                    <option value="coin-m">{pickText(lang, "币本位合约", "COIN-M futures")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "策略模式", "Strategy mode")}>
                  <Select defaultValue="classic" name="mode">
                    <option value="classic">{pickText(lang, "双向现货网格", "Classic two-way spot")}</option>
                    <option value="buy-only">{pickText(lang, "只买现货网格", "Buy-only spot")}</option>
                    <option value="sell-only">{pickText(lang, "只卖现货网格", "Sell-only spot")}</option>
                    <option value="long">{pickText(lang, "做多合约网格", "Long futures")}</option>
                    <option value="short">{pickText(lang, "做空合约网格", "Short futures")}</option>
                    <option value="neutral">{pickText(lang, "中性合约网格", "Neutral futures")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "生成方式", "Generation mode")}>
                  <Select defaultValue="arithmetic" name="generation">
                    <option value="arithmetic">{pickText(lang, "等差", "Arithmetic")}</option>
                    <option value="geometric">{pickText(lang, "等比", "Geometric")}</option>
                    <option value="custom">{pickText(lang, "完全自定义", "Fully custom")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "编辑模式", "Editor mode")} hint={pickText(lang, "批量模式适合快速出梯；JSON 适合逐格定制。", "Batch mode is fast for ladder generation; JSON keeps full per-level control.")}>
                  <Select defaultValue="batch" name="editorMode">
                    <option value="batch">{pickText(lang, "批量建梯", "Batch ladder builder")}</option>
                    <option value="custom">{pickText(lang, "自定义 JSON", "Custom JSON")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "金额模式", "Amount mode")} hint={pickText(lang, "USDT 金额和币数量二选一。", "Choose quote amount or base quantity.")}>
                  <Select defaultValue="quote" name="amountMode">
                    <option value="quote">{pickText(lang, "计价金额", "Quote amount")}</option>
                    <option value="base">{pickText(lang, "基础币数量", "Base quantity")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "合约保证金模式", "Futures margin mode")} hint={pickText(lang, "仅合约策略生效。", "Applies only to futures strategies.")}>
                  <Select defaultValue="isolated" name="futuresMarginMode">
                    <option value="isolated">{pickText(lang, "逐仓", "Isolated")}</option>
                    <option value="cross">{pickText(lang, "全仓", "Cross")}</option>
                  </Select>
                </Field>
                <Field label={pickText(lang, "杠杆", "Leverage")}>
                  <Input defaultValue="5" inputMode="numeric" name="leverage" />
                </Field>
                <div className="content-grid content-grid--split">
                  <Field label={pickText(lang, "计价金额 (USDT)", "Quote amount (USDT)")}>
                    <Input defaultValue="1200" inputMode="decimal" name="quoteAmount" />
                  </Field>
                  <Field label={pickText(lang, "基础币数量", "Base asset quantity")}>
                    <Input defaultValue="0.0100" inputMode="decimal" name="baseQuantity" />
                  </Field>
                </div>
                <div className="content-grid content-grid--split">
                  <Field label={pickText(lang, "参考价", "Reference price")}>
                    <Input defaultValue="100" inputMode="decimal" name="referencePrice" />
                  </Field>
                  <Field label={pickText(lang, "网格数量", "Grid count")}>
                    <Input defaultValue="4" inputMode="numeric" name="gridCount" />
                  </Field>
                </div>
                <div className="content-grid content-grid--split">
                  <Field label={pickText(lang, "网格间距 (%)", "Batch spacing (%)")}>
                    <Input defaultValue="1.5" inputMode="decimal" name="gridSpacingPercent" />
                  </Field>
                  <Field label={pickText(lang, "单格止盈 (%)", "Batch take profit (%)")}>
                    <Input defaultValue="2.2" inputMode="decimal" name="batchTakeProfit" />
                  </Field>
                </div>
                <div className="content-grid content-grid--split">
                  <Field label={pickText(lang, "移动止盈 (%)", "Trailing take profit (%)")}>
                    <Input defaultValue="0.7" inputMode="decimal" name="batchTrailing" />
                  </Field>
                  <Field label={pickText(lang, "总体止盈 (%)", "Overall take profit (%)")}>
                    <Input defaultValue="7.0" inputMode="decimal" name="overallTakeProfit" required />
                  </Field>
                </div>
                <Field label={pickText(lang, "总体止损 (%)", "Overall stop loss (%)")}>
                  <Input defaultValue="2.5" inputMode="decimal" name="overallStopLoss" />
                </Field>
                <Field label={pickText(lang, "网格 JSON", "Grid levels JSON")} hint={pickText(lang, "需要逐格覆盖时再切到 JSON。", "Use JSON only when per-level overrides are required.")}>
                  <textarea className="ui-input" defaultValue={defaultLevelsJson} name="levels_json" rows={10} />
                </Field>
                <Field label={pickText(lang, "触发后行为", "Post-trigger behavior")}>
                  <Select defaultValue="rebuild" name="postTrigger">
                    <option value="stop">{pickText(lang, "执行后停止", "Stop after execution")}</option>
                    <option value="rebuild">{pickText(lang, "重建并继续", "Rebuild and continue")}</option>
                  </Select>
                </Field>
                <Button type="submit">{pickText(lang, "保存草稿", "Save draft")}</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>{pickText(lang, "策略模板", "Strategy templates")}</CardTitle>
              <CardDescription>{pickText(lang, "模板复用、容量提醒和预检要点统一放到右侧。", "Template reuse, capacity reminders, and pre-flight notes live on the side rail.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/strategies/templates" method="post">
                <Field label={pickText(lang, "模板生成的策略名", "Draft name from template")}>
                  <Input defaultValue="Template Based Draft" name="name" required />
                </Field>
                <Field label={pickText(lang, "应用模板", "Apply template")}>
                  <Select defaultValue={templates[0]?.id ?? ""} name="templateId">
                    {templates.length === 0 ? <option value="">{pickText(lang, "暂无模板", "No templates available")}</option> : null}
                    {templates.map((template) => (
                      <option key={template.id} value={template.id}>{template.name + " · " + template.symbol + " · " + template.market}</option>
                    ))}
                  </Select>
                </Field>
                <Button type="submit">{pickText(lang, "套用模板", "Apply template")}</Button>
              </FormStack>
              <ul className="text-list">
                <li>{pickText(lang, "当前用户草稿数", "Existing user drafts")}: {strategies.length}</li>
                <li>{pickText(lang, "合约方向同一用户同一标的只能有一个实例。", "Futures allow only one strategy per user, symbol, and direction.")}</li>
                <li>{pickText(lang, "移动止盈走 taker，成本会高于普通挂单。", "Trailing take profit uses taker execution and may cost more than passive orders.")}</li>
                <li>{pickText(lang, "保存草稿后仍需到工作台执行预检与启动。", "After saving the draft, pre-flight and launch still happen in the workspace.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <DialogFrame
        description={pickText(lang, "未通过预检前不能启动；预检会检查余额、交易所过滤器与对冲模式。", "Launch remains blocked until pre-flight confirms balance, exchange filters, and hedge mode.")}
        title={pickText(lang, "预检仍然是强制步骤", "Pre-flight remains mandatory")}
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
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
  const response = await fetch(authApiBaseUrl() + "/strategies/templates", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
  const response = await fetch(authApiBaseUrl() + "/exchange/binance/symbols/search", {
    method: "POST",
    headers: {
      authorization: "Bearer " + sessionToken,
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
