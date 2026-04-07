import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage, type UiLanguage } from "../../../lib/ui/preferences";

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
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const params = (await searchParams) ?? {};
  const notice = firstValue(params.notice);
  const error = firstValue(params.error);
  const statusFilter = firstValue(params.status) ?? "all";
  const symbolFilter = firstValue(params.symbol) ?? "";
  const strategyResult = await fetchStrategies();
  const strategies = strategyResult.items;
  const filteredStrategies = strategies.filter((item) => {
    const statusMatches = statusFilter === "all" || item.status === statusFilter;
    const query = symbolFilter.trim().toLowerCase();
    const symbolMatches = !query || item.symbol.toLowerCase().includes(query) || item.name.toLowerCase().includes(query);
    return statusMatches && symbolMatches;
  });
  const selectedIds = filteredStrategies.map((item) => item.id);
  const summaryBar = [
    { label: pickText(lang, "草稿", "Drafts"), value: String(strategies.filter((item) => item.status === "Draft").length) },
    { label: pickText(lang, "运行中", "Running"), value: String(strategies.filter((item) => item.status === "Running").length) },
    { label: pickText(lang, "已暂停", "Paused"), value: String(strategies.filter((item) => item.status === "Paused").length) },
    { label: pickText(lang, "异常暂停", "Blocked"), value: String(strategies.filter((item) => item.status === "ErrorPaused").length) },
    { label: pickText(lang, "过滤后", "Filtered"), value: String(filteredStrategies.length) },
  ];

  return (
    <>
      <StatusBanner
        description={pickText(lang, "策略页优先展示筛选、批量动作和真实状态，不把关键动作埋进说明文案。", "The strategy page prioritizes filters, batch actions, and real runtime state over passive copy.")}
        title={pickText(lang, "策略目录状态条", "Strategies status strip")}
        tone="warning"
      />
      {notice ? <StatusBanner description={formatNotice(lang, notice)} title={pickText(lang, "策略更新成功", "Strategy updated")} tone="success" /> : null}
      {error ? <StatusBanner description={error} title={pickText(lang, "策略操作失败", "Strategy action failed")} tone="warning" /> : null}
      {strategyResult.error ? <StatusBanner description={strategyResult.error} title={pickText(lang, "策略列表暂不可用", "Strategy catalog unavailable")} tone="warning" /> : null}
      <AppShellSection
        actions={
          <div className="button-row" style={{ position: "sticky", top: "1rem", zIndex: 2 }}>
            <form action="/api/user/strategies/batch" method="post">
              <input name="intent" type="hidden" value="stop-all" />
              <Button type="submit">{pickText(lang, "全部停机", "Stop all")}</Button>
            </form>
            <Link className="button button--ghost" href="/app/strategies/new">
              {pickText(lang, "新建策略", "New strategy")}
            </Link>
          </div>
        }
        description={pickText(lang, "吸顶工具栏处理筛选与全局动作，主表格只保留交易相关信息。", "The sticky toolbar owns filtering and global actions so the table can stay trading-focused.")}
        eyebrow={pickText(lang, "策略目录", "Strategy catalog")}
        title={pickText(lang, "策略台账", "Strategies")}
      >
        <div style={{ position: "sticky", top: "4.5rem", zIndex: 1 }}>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "吸顶工具栏", "Sticky toolbar")}</CardTitle>
              <CardDescription>{pickText(lang, "先筛选再批量执行，避免对不可见行误操作。", "Filter first, then batch-operate, so hidden rows are not touched by mistake.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/app/strategies" method="get">
                <div className="content-grid content-grid--split">
                  <Field label={pickText(lang, "状态筛选", "Status filter")}>
                    <Select defaultValue={statusFilter} name="status">
                      <option value="all">{pickText(lang, "全部", "All")}</option>
                      <option value="Draft">{pickText(lang, "草稿", "Draft")}</option>
                      <option value="Running">{pickText(lang, "运行中", "Running")}</option>
                      <option value="Paused">{pickText(lang, "已暂停", "Paused")}</option>
                      <option value="ErrorPaused">{pickText(lang, "异常暂停", "Blocked")}</option>
                      <option value="Stopped">{pickText(lang, "已停止", "Stopped")}</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "交易对或策略名", "Symbol or strategy")}>
                    <Input defaultValue={symbolFilter} name="symbol" />
                  </Field>
                </div>
                <div className="button-row">
                  <Button type="submit">{pickText(lang, "应用筛选", "Apply filter")}</Button>
                  <Link className="button button--ghost" href="/app/orders">{pickText(lang, "查看订单历史", "Open orders history")}</Link>
                </div>
              </FormStack>
            </CardBody>
          </Card>
        </div>
        <div className="content-grid content-grid--metrics">
          {summaryBar.map((summary) => (
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
            <CardTitle>{pickText(lang, "主表格", "Strategy inventory")}</CardTitle>
            <CardDescription>{pickText(lang, "只展示用户当前可操作的运行、预算、市场和状态。", "Show only the state, market, and exposure the user can act on right now.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <form action="/api/user/strategies/batch" method="post">
              <DataTable
                columns={[
                  { key: "pick", label: pickText(lang, "选择", "Select") },
                  { key: "name", label: pickText(lang, "策略", "Strategy") },
                  { key: "market", label: pickText(lang, "市场", "Market") },
                  { key: "state", label: pickText(lang, "状态", "State") },
                  { key: "exposure", label: pickText(lang, "预算", "Exposure"), align: "right" },
                ]}
                rows={filteredStrategies.map((row) => ({
                  id: row.id,
                  pick: <input aria-label={pickText(lang, "选择 " + row.name, "Select " + row.name)} name="ids" type="checkbox" value={row.id} />,
                  name: <Link href={"/app/strategies/" + row.id}>{row.name}</Link>,
                  market: describeMarket(lang, row.market),
                  exposure: row.budget,
                  state: <Chip tone={statusTone(row.status)}>{describeStrategyStatus(lang, row.status)}</Chip>,
                }))}
              />
              <div className="button-row" style={{ position: "sticky", bottom: "1rem", zIndex: 2, marginTop: "1rem" }}>
                <Button name="intent" type="submit" value="start">{pickText(lang, "启动选中项", "Start selected")}</Button>
                <Button name="intent" tone="secondary" type="submit" value="pause">{pickText(lang, "暂停选中项", "Pause selected")}</Button>
                <Button name="intent" tone="secondary" type="submit" value="delete">{pickText(lang, "删除选中项", "Delete selected")}</Button>
              </div>
            </form>
            <form action="/api/user/strategies/batch" method="post">
              {selectedIds.map((id) => (
                <input key={"filtered-" + id} name="ids" type="hidden" value={id} />
              ))}
              <div className="button-row">
                <Button name="intent" type="submit" value="start">{pickText(lang, "启动筛选结果", "Start filtered")}</Button>
                <Button name="intent" tone="secondary" type="submit" value="pause">{pickText(lang, "暂停筛选结果", "Pause filtered")}</Button>
                <Button name="intent" tone="secondary" type="submit" value="delete">{pickText(lang, "删除筛选结果", "Delete filtered")}</Button>
              </div>
            </form>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>{pickText(lang, "操作规则", "Batch rules")}</CardTitle>
            <CardDescription>{pickText(lang, "批量栏是 sticky 的，但每条规则仍由后端接口最终裁决。", "The batch bar is sticky, but backend rules still decide what can actually run.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "策略创建先进入草稿，再预检，再启动。", "Every strategy starts as a draft, then pre-flights, then launches.")}</li>
              <li>{pickText(lang, "正在运行的策略必须先暂停，才能编辑并保存。", "Running strategies must be paused before edits can be saved.")}</li>
              <li>{pickText(lang, "删除前要清空挂单与仓位。", "Deletion requires both working orders and positions to be cleared.")}</li>
              <li>{pickText(lang, "异常暂停会触发 Telegram 与站内提醒。", "Blocked runtime exceptions trigger Telegram and in-app notifications.")}</li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}

function describeMarket(lang: UiLanguage, market: string) {
  switch (market) {
    case "Spot":
      return pickText(lang, "现货", "Spot");
    case "FuturesUsdM":
      return pickText(lang, "U 本位合约", "USD-M futures");
    case "FuturesCoinM":
      return pickText(lang, "币本位合约", "COIN-M futures");
    default:
      return market;
  }
}

function describeStrategyStatus(lang: UiLanguage, status: string) {
  switch (status) {
    case "Draft":
      return pickText(lang, "草稿", "Draft");
    case "Running":
      return pickText(lang, "运行中", "Running");
    case "Paused":
      return pickText(lang, "已暂停", "Paused");
    case "ErrorPaused":
      return pickText(lang, "异常暂停", "Blocked");
    case "Stopped":
      return pickText(lang, "已停止", "Stopped");
    default:
      return status;
  }
}

function statusTone(status: string) {
  if (status === "Running") return "success" as const;
  if (status === "Paused") return "warning" as const;
  if (status === "ErrorPaused") return "danger" as const;
  return "info" as const;
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
    return { items: [], error: pickText(resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value), "策略列表暂时不可用。", "Strategy catalog is temporarily unavailable.") };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function formatNotice(lang: UiLanguage, value: string) {
  const parts = value.split("-");
  if (parts.length === 0) {
    return value;
  }
  const first = parts[0] === "preflight" ? pickText(lang, "预检", "Pre-flight") : pickText(lang, parts[0], parts[0].charAt(0).toUpperCase() + parts[0].slice(1));
  return [first].concat(parts.slice(1)).join(" ");
}
