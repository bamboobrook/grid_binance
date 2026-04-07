import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { getAdminTemplatesData, getCurrentAdminProfile } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type PageProps = {
  searchParams?: Promise<{ created?: string; edit?: string; updated?: string }>;
};

function readinessOptions(lang: "zh" | "en") {
  return (
    <>
      <option value="true">{pickText(lang, "就绪", "Ready")}</option>
      <option value="false">{pickText(lang, "未就绪", "Not Ready")}</option>
    </>
  );
}

export default async function AdminTemplatesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const [cookieStore, profile] = await Promise.all([cookies(), getCurrentAdminProfile()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const created = typeof params.created === "string" ? params.created : "";
  const updated = typeof params.updated === "string" ? params.updated : "";
  const editId = typeof params.edit === "string" ? params.edit : "";
  if (profile.admin_role !== "super_admin") {
    redirect("/admin/dashboard");
  }

  const canManageTemplates = profile.admin_permissions?.can_manage_templates ?? false;
  const data = canManageTemplates ? await getAdminTemplatesData() : { items: [] };
  const editingTemplate = editId ? data.items.find((item) => item.id === editId) ?? null : null;
  const levelOne = editingTemplate?.levels[0] ?? null;
  const levelTwo = editingTemplate?.levels[1] ?? null;
  const levelsJson = JSON.stringify(editingTemplate?.levels ?? [
    { entry_price: "1.0000", quantity: "10", take_profit_bps: 150, trailing_bps: null },
    { entry_price: "1.0500", quantity: "10", take_profit_bps: 180, trailing_bps: null },
    { entry_price: "1.1000", quantity: "12", take_profit_bps: 220, trailing_bps: 90 },
  ], null, 2);
  const amountMode = editingTemplate?.amount_mode === "Base" ? "base" : "quote";
  const futuresMarginMode = editingTemplate?.futures_margin_mode === "Cross" ? "cross" : "isolated";
  const leverage = editingTemplate?.leverage ? String(editingTemplate.leverage) : "5";

  return (
    <>
      {created ? <StatusBanner description={pickText(lang, "已创建模板：" + created, "Created template: " + created)} title={pickText(lang, "模板已创建", "Template Created")} tone="success" /> : null}
      {updated ? <StatusBanner description={pickText(lang, "已更新模板：" + updated, "Updated template: " + updated)} title={pickText(lang, "模板已更新", "Template Updated")} tone="success" /> : null}
      <AppShellSection
        description={pickText(lang, "值班席位在这里核对模板就绪门禁、层级配置和未来可复制的策略模板。", "The desk reviews template readiness gates, ladder settings, and future-copy strategy templates here.")}
        eyebrow={pickText(lang, "模板治理", "Template Governance")}
        title={pickText(lang, "模板管理", "Template Management")}
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{editingTemplate ? pickText(lang, "编辑模板", "Edit Template") : pickText(lang, "创建模板", "Create Template")}</CardTitle>
              <CardDescription>{pickText(lang, "模板变更只影响未来复制，不会回写已经应用到用户上的策略。", "Template updates affect future copies only and do not mutate already-applied user strategies.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {canManageTemplates ? (
                <FormStack action="/api/admin/templates" method="post">
                  {editingTemplate ? <input name="intent" type="hidden" value="update" /> : null}
                  {editingTemplate ? <input name="templateId" type="hidden" value={editingTemplate.id} /> : null}
                  <Field label={pickText(lang, "模板名称", "Template Name")}>
                    <Input defaultValue={editingTemplate?.name ?? ""} name="name" placeholder="ADA Trend Rider" />
                  </Field>
                  <Field label={pickText(lang, "交易对", "Symbol")}>
                    <Input defaultValue={editingTemplate?.symbol ?? "ADAUSDT"} name="symbol" />
                  </Field>
                  <Field label={pickText(lang, "市场", "Market")}>
                    <Select defaultValue={editingTemplate?.market ?? "Spot"} name="market">
                      <option value="Spot">{pickText(lang, "现货", "Spot")}</option>
                      <option value="FuturesUsdM">USDⓈ-M</option>
                      <option value="FuturesCoinM">COIN-M</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "模式", "Mode")}>
                    <Select defaultValue={editingTemplate?.mode ?? "SpotClassic"} name="mode">
                      <option value="SpotClassic">{pickText(lang, "现货经典", "Spot Classic")}</option>
                      <option value="SpotBuyOnly">{pickText(lang, "现货只买", "Spot Buy Only")}</option>
                      <option value="SpotSellOnly">{pickText(lang, "现货只卖", "Spot Sell Only")}</option>
                      <option value="FuturesLong">{pickText(lang, "合约做多", "Futures Long")}</option>
                      <option value="FuturesShort">{pickText(lang, "合约做空", "Futures Short")}</option>
                      <option value="FuturesNeutral">{pickText(lang, "合约中性", "Futures Neutral")}</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "生成方式", "Generation")}>
                    <Select defaultValue={editingTemplate?.generation ?? "Custom"} name="generation">
                      <option value="Arithmetic">{pickText(lang, "等差", "Arithmetic")}</option>
                      <option value="Geometric">{pickText(lang, "等比", "Geometric")}</option>
                      <option value="Custom">{pickText(lang, "自定义", "Custom")}</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "计量模式", "Amount Mode")}>
                    <Select defaultValue={amountMode} name="amountMode">
                      <option value="quote">{pickText(lang, "报价资产", "Quote")}</option>
                      <option value="base">{pickText(lang, "基础资产", "Base")}</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "合约保证金模式", "Futures Margin Mode")}>
                    <Select defaultValue={futuresMarginMode} name="futuresMarginMode">
                      <option value="isolated">{pickText(lang, "逐仓", "Isolated")}</option>
                      <option value="cross">{pickText(lang, "全仓", "Cross")}</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "杠杆", "Leverage")}>
                    <Input defaultValue={leverage} inputMode="numeric" name="leverage" />
                  </Field>
                  <Field label={pickText(lang, "层级 1 入场价", "Level 1 Entry Price")}>
                    <Input defaultValue={levelOne?.entry_price ?? "1.0000"} name="level1EntryPrice" />
                  </Field>
                  <Field label={pickText(lang, "层级 1 数量", "Level 1 Quantity")}>
                    <Input defaultValue={levelOne?.quantity ?? "10"} name="level1Quantity" />
                  </Field>
                  <Field label={pickText(lang, "层级 1 止盈 bps", "Level 1 Take Profit (bps)")}>
                    <Input defaultValue={String(levelOne?.take_profit_bps ?? 150)} inputMode="numeric" name="level1TakeProfitBps" />
                  </Field>
                  <Field label={pickText(lang, "层级 1 追踪 bps", "Level 1 Trailing (bps)")}>
                    <Input defaultValue={levelOne?.trailing_bps ?? ""} name="level1TrailingBps" />
                  </Field>
                  <Field label={pickText(lang, "层级 2 入场价", "Level 2 Entry Price")}>
                    <Input defaultValue={levelTwo?.entry_price ?? "1.1000"} name="level2EntryPrice" />
                  </Field>
                  <Field label={pickText(lang, "层级 2 数量", "Level 2 Quantity")}>
                    <Input defaultValue={levelTwo?.quantity ?? "10"} name="level2Quantity" />
                  </Field>
                  <Field label={pickText(lang, "层级 2 止盈 bps", "Level 2 Take Profit (bps)")}>
                    <Input defaultValue={String(levelTwo?.take_profit_bps ?? 180)} inputMode="numeric" name="level2TakeProfitBps" />
                  </Field>
                  <Field label={pickText(lang, "层级 2 追踪 bps", "Level 2 Trailing (bps)")}>
                    <Input defaultValue={levelTwo?.trailing_bps ?? ""} name="level2TrailingBps" />
                  </Field>
                  <Field label={pickText(lang, "会员门禁", "Membership Ready")}>
                    <Select defaultValue={String(editingTemplate?.membership_ready ?? true)} name="membershipReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "交易所门禁", "Exchange Ready")}>
                    <Select defaultValue={String(editingTemplate?.exchange_ready ?? true)} name="exchangeReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "权限门禁", "Permissions Ready")}>
                    <Select defaultValue={String(editingTemplate?.permissions_ready ?? true)} name="permissionsReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "禁提门禁", "Withdrawals Disabled")}>
                    <Select defaultValue={String(editingTemplate?.withdrawals_disabled ?? true)} name="withdrawalsDisabled">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "双向持仓门禁", "Hedge Mode Ready")}>
                    <Select defaultValue={String(editingTemplate?.hedge_mode_ready ?? true)} name="hedgeModeReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "交易对门禁", "Symbol Ready")}>
                    <Select defaultValue={String(editingTemplate?.symbol_ready ?? true)} name="symbolReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "过滤器门禁", "Filters Ready")}>
                    <Select defaultValue={String(editingTemplate?.filters_ready ?? true)} name="filtersReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "保证金门禁", "Margin Ready")}>
                    <Select defaultValue={String(editingTemplate?.margin_ready ?? true)} name="marginReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "冲突门禁", "Conflict Ready")}>
                    <Select defaultValue={String(editingTemplate?.conflict_ready ?? true)} name="conflictReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "余额门禁", "Balance Ready")}>
                    <Select defaultValue={String(editingTemplate?.balance_ready ?? true)} name="balanceReady">{readinessOptions(lang)}</Select>
                  </Field>
                  <Field label={pickText(lang, "层级 JSON", "Levels JSON")} hint={pickText(lang, "当模板需要 3 层以上或完全自定义阶梯时使用。", "Use this field when the template needs 3+ levels or full custom ladder control.")}>
                    <textarea className="ui-input" defaultValue={levelsJson} name="levelsJson" rows={10} />
                  </Field>
                  <Field label={pickText(lang, "整体止盈 bps", "Overall Take Profit (bps)")}>
                    <Input defaultValue={editingTemplate?.overall_take_profit_bps ?? ""} inputMode="numeric" name="overallTakeProfitBps" />
                  </Field>
                  <Field label={pickText(lang, "整体止损 bps", "Overall Stop Loss (bps)")}>
                    <Input defaultValue={editingTemplate?.overall_stop_loss_bps ?? ""} inputMode="numeric" name="overallStopLossBps" />
                  </Field>
                  <Field label={pickText(lang, "触发后动作", "Post-trigger Action")}>
                    <Select defaultValue={editingTemplate?.post_trigger_action ?? "Stop"} name="postTriggerAction">
                      <option value="Stop">{pickText(lang, "停止", "Stop")}</option>
                      <option value="Rebuild">{pickText(lang, "重建", "Rebuild")}</option>
                    </Select>
                  </Field>
                  <Button type="submit">{editingTemplate ? pickText(lang, "保存模板变更", "Save Template Changes") : pickText(lang, "创建模板", "Create Template")}</Button>
                </FormStack>
              ) : (
                <p>{pickText(lang, "模板变更需要 super_admin。", "Template changes require super_admin.")}</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "模板清单", "Template Inventory")}</CardTitle>
          <CardDescription>{pickText(lang, "模板清单会显式暴露市场、生成方式和层级数量。", "The inventory exposes market, generation, and ladder depth explicitly.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={canManageTemplates ? [
              { key: "name", label: pickText(lang, "名称", "Name") },
              { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
              { key: "market", label: pickText(lang, "市场", "Market") },
              { key: "generation", label: pickText(lang, "生成方式", "Generation") },
              { key: "levels", label: pickText(lang, "层级数", "Levels") },
              { key: "actions", label: pickText(lang, "动作", "Actions") },
            ] : [
              { key: "name", label: pickText(lang, "名称", "Name") },
              { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
              { key: "market", label: pickText(lang, "市场", "Market") },
              { key: "generation", label: pickText(lang, "生成方式", "Generation") },
              { key: "levels", label: pickText(lang, "层级数", "Levels") },
            ]}
            rows={data.items.map((item) => ({
              actions: canManageTemplates ? (
                <form action="/admin/templates" method="get">
                  <input name="edit" type="hidden" value={item.id} />
                  <Button type="submit">{pickText(lang, "编辑模板", "Edit Template")}</Button>
                </form>
              ) : null,
              generation: item.generation,
              id: item.id,
              levels: pickText(lang, String(item.levels.length) + " 层", String(item.levels.length) + " levels"),
              market: item.market,
              name: item.name,
              symbol: item.symbol,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
