import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, ButtonRow, Field, FormStack, Select } from "@/components/ui/form";
import { DataTable } from "@/components/ui/table";
import { getAdminStrategiesData } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

type PageProps = {
  searchParams?: Promise<{ selected?: string; state?: string }>;
};

function statusLabel(lang: UiLanguage, status: string) {
  switch (status) {
    case "Draft":
      return pickText(lang, "草稿", "Draft");
    case "Running":
      return pickText(lang, "运行中", "Running");
    case "Paused":
      return pickText(lang, "已暂停", "Paused");
    case "ErrorPaused":
      return pickText(lang, "异常暂停", "Error-paused");
    default:
      return status;
  }
}

export default async function AdminStrategiesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const [cookieStore, data] = await Promise.all([cookies(), getAdminStrategiesData()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const stateFilter = typeof params.state === "string" ? params.state.toLowerCase() : "all";
  const selectedId = typeof params.selected === "string" ? params.selected : "";
  const items = data.items.filter((item) => (stateFilter === "all" ? true : item.status.toLowerCase() == stateFilter));
  const selected = data.items.find((item) => item.id === selectedId) ?? items[0] ?? null;

  return (
    <>
      <AppShellSection
        description={pickText(lang, "值班席位按运行态筛选策略，并通过明确的选中详情面板查看预检、委托与运行事件。", "The desk filters strategies by runtime state and uses an explicit selected-detail panel for pre-flight, orders, and runtime events.")}
        eyebrow={pickText(lang, "策略监督", "Strategy Supervision")}
        title={pickText(lang, "策略总览", "Strategy Oversight")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "运行态筛选", "Runtime Filter")}</CardTitle>
              <CardDescription>{pickText(lang, "按运行状态缩小值班视野。", "Filter backend strategies by runtime state.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/strategies" method="get">
                <Field label={pickText(lang, "运行态", "Runtime State")}>
                  <Select defaultValue={stateFilter} name="state">
                    <option value="all">{pickText(lang, "全部状态", "All States")}</option>
                    <option value="draft">{pickText(lang, "草稿", "Draft")}</option>
                    <option value="running">{pickText(lang, "运行中", "Running")}</option>
                    <option value="paused">{pickText(lang, "已暂停", "Paused")}</option>
                    <option value="errorpaused">{pickText(lang, "异常暂停", "Error-paused")}</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">{pickText(lang, "应用筛选", "Apply Filters")}</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "选中详情", "Selected Detail")}</CardTitle>
              <CardDescription>{pickText(lang, "不再默认盯住第一条；选中态通过 query 显式保留。", "The page no longer blindly focuses the first row; selection is kept explicitly through the query string.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {selected ? (
                <ul className="text-list">
                  <li>{pickText(lang, "名称：", "Name: ") + selected.name}</li>
                  <li>{pickText(lang, "所有者：", "Owner: ") + selected.owner_email}</li>
                  <li>{pickText(lang, "运行态：", "Runtime State: ") + statusLabel(lang, selected.status)}</li>
                  <li>{pickText(lang, "预检：", "Pre-flight: ") + (selected.runtime.last_preflight ? (selected.runtime.last_preflight.ok ? pickText(lang, "通过", "Passed") : pickText(lang, "失败", "Failed")) : pickText(lang, "缺失", "Missing"))}</li>
                  <li>{pickText(lang, "活动委托数：", "Active Orders: ") + String(selected.runtime.orders.length)}</li>
                </ul>
              ) : (
                <p>{pickText(lang, "暂无可见策略。", "No operator-visible strategies yet.")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "策略清单", "Strategy Inventory")}</CardTitle>
          <CardDescription>{pickText(lang, "每行都提供进入选中详情的入口。", "Every row exposes an explicit entry into the selected-detail panel.")}</CardDescription>
        </CardHeader>
        <CardBody>
          {items.length === 0 ? (
            <p>{pickText(lang, "暂无可见策略。", "No operator-visible strategies yet.")}</p>
          ) : (
            <DataTable
              columns={[
                { key: "name", label: pickText(lang, "名称", "Name") },
                { key: "owner", label: pickText(lang, "所有者", "Owner") },
                { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                { key: "status", label: pickText(lang, "状态", "Status") },
                { key: "orders", label: pickText(lang, "活动委托", "Active Orders") },
                { key: "preflight", label: pickText(lang, "预检", "Pre-flight") },
                { key: "action", label: pickText(lang, "动作", "Action") },
              ]}
              rows={items.map((item) => ({
                id: item.id,
                action: (
                  <form action="/admin/strategies" method="get">
                    <input name="state" type="hidden" value={stateFilter} />
                    <input name="selected" type="hidden" value={item.id} />
                    <Button type="submit">{pickText(lang, "查看详情", "View Detail")}</Button>
                  </form>
                ),
                name: item.name,
                orders: String(item.runtime.orders.length),
                owner: item.owner_email,
                preflight: item.runtime.last_preflight ? (item.runtime.last_preflight.ok ? pickText(lang, "通过", "Passed") : pickText(lang, "失败", "Failed")) : pickText(lang, "缺失", "Missing"),
                status: statusLabel(lang, item.status),
                symbol: item.symbol,
              }))}
            />
          )}
        </CardBody>
      </Card>
    </>
  );
}
