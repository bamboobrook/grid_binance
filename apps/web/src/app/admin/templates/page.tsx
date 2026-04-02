import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminTemplatesData, getCurrentAdminProfile } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ created?: string; edit?: string; updated?: string }>;
};

function readinessOptions() {
  return (
    <>
      <option value="true">true</option>
      <option value="false">false</option>
    </>
  );
}

export default async function AdminTemplatesPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const created = typeof params.created === "string" ? params.created : "";
  const updated = typeof params.updated === "string" ? params.updated : "";
  const editId = typeof params.edit === "string" ? params.edit : "";
  const profile = await getCurrentAdminProfile();
  const canManageTemplates = profile.admin_permissions?.can_manage_templates ?? false;
  const data = canManageTemplates ? await getAdminTemplatesData() : { items: [] };
  const editingTemplate = editId ? data.items.find((item) => item.id === editId) ?? null : null;
  const levelOne = editingTemplate?.levels[0] ?? null;
  const levelTwo = editingTemplate?.levels[1] ?? null;

  return (
    <>
      {created ? <StatusBanner description={`Created template name: ${created}`} title="Template created" tone="success" /> : null}
      {updated ? <StatusBanner description={`Updated template name: ${updated}`} title="Template updated" tone="success" /> : null}
      <AppShellSection
        description="Template inventory is read from the backend strategy template catalog."
        eyebrow="Strategy templates"
        title="Template Management"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{editingTemplate ? "Edit template" : "Create template"}</CardTitle>
              <CardDescription>Template updates affect future copies only and do not mutate already-applied user strategies.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManageTemplates ? (
                <FormStack action="/api/admin/templates" method="post">
                  {editingTemplate ? <input name="intent" type="hidden" value="update" /> : null}
                  {editingTemplate ? <input name="templateId" type="hidden" value={editingTemplate.id} /> : null}
                  <Field label="Template name">
                    <Input defaultValue={editingTemplate?.name ?? ""} name="name" placeholder="ADA Trend Rider" />
                  </Field>
                  <Field label="Symbol">
                    <Input defaultValue={editingTemplate?.symbol ?? "ADAUSDT"} name="symbol" />
                  </Field>
                  <Field label="Market">
                    <Select defaultValue={editingTemplate?.market ?? "Spot"} name="market">
                      <option value="Spot">Spot</option>
                      <option value="FuturesUsdM">FuturesUsdM</option>
                      <option value="FuturesCoinM">FuturesCoinM</option>
                    </Select>
                  </Field>
                  <Field label="Mode">
                    <Select defaultValue={editingTemplate?.mode ?? "SpotClassic"} name="mode">
                      <option value="SpotClassic">SpotClassic</option>
                      <option value="SpotBuyOnly">SpotBuyOnly</option>
                      <option value="SpotSellOnly">SpotSellOnly</option>
                      <option value="FuturesLong">FuturesLong</option>
                      <option value="FuturesShort">FuturesShort</option>
                      <option value="FuturesNeutral">FuturesNeutral</option>
                    </Select>
                  </Field>
                  <Field label="Generation">
                    <Select defaultValue={editingTemplate?.generation ?? "Custom"} name="generation">
                      <option value="Arithmetic">Arithmetic</option>
                      <option value="Geometric">Geometric</option>
                      <option value="Custom">Custom</option>
                    </Select>
                  </Field>
                  <Field label="Level 1 entry price">
                    <Input defaultValue={levelOne?.entry_price ?? "1.0000"} name="level1EntryPrice" />
                  </Field>
                  <Field label="Level 1 quantity">
                    <Input defaultValue={levelOne?.quantity ?? "10"} name="level1Quantity" />
                  </Field>
                  <Field label="Level 1 take profit (bps)">
                    <Input defaultValue={String(levelOne?.take_profit_bps ?? 150)} inputMode="numeric" name="level1TakeProfitBps" />
                  </Field>
                  <Field label="Level 1 trailing (bps)">
                    <Input defaultValue={levelOne?.trailing_bps ?? ""} name="level1TrailingBps" />
                  </Field>
                  <Field label="Level 2 entry price">
                    <Input defaultValue={levelTwo?.entry_price ?? "1.1000"} name="level2EntryPrice" />
                  </Field>
                  <Field label="Level 2 quantity">
                    <Input defaultValue={levelTwo?.quantity ?? "10"} name="level2Quantity" />
                  </Field>
                  <Field label="Level 2 take profit (bps)">
                    <Input defaultValue={String(levelTwo?.take_profit_bps ?? 180)} inputMode="numeric" name="level2TakeProfitBps" />
                  </Field>
                  <Field label="Level 2 trailing (bps)">
                    <Input defaultValue={levelTwo?.trailing_bps ?? ""} name="level2TrailingBps" />
                  </Field>
                  <Field label="Membership ready">
                    <Select defaultValue={String(editingTemplate?.membership_ready ?? true)} name="membershipReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Exchange ready">
                    <Select defaultValue={String(editingTemplate?.exchange_ready ?? true)} name="exchangeReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Permissions ready">
                    <Select defaultValue={String(editingTemplate?.permissions_ready ?? true)} name="permissionsReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Withdrawals disabled">
                    <Select defaultValue={String(editingTemplate?.withdrawals_disabled ?? true)} name="withdrawalsDisabled">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Hedge mode ready">
                    <Select defaultValue={String(editingTemplate?.hedge_mode_ready ?? true)} name="hedgeModeReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Symbol ready">
                    <Select defaultValue={String(editingTemplate?.symbol_ready ?? true)} name="symbolReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Filters ready">
                    <Select defaultValue={String(editingTemplate?.filters_ready ?? true)} name="filtersReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Margin ready">
                    <Select defaultValue={String(editingTemplate?.margin_ready ?? true)} name="marginReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Conflict ready">
                    <Select defaultValue={String(editingTemplate?.conflict_ready ?? true)} name="conflictReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Balance ready">
                    <Select defaultValue={String(editingTemplate?.balance_ready ?? true)} name="balanceReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Overall take profit (bps)">
                    <Input defaultValue={editingTemplate?.overall_take_profit_bps ?? ""} inputMode="numeric" name="overallTakeProfitBps" />
                  </Field>
                  <Field label="Overall stop loss (bps)">
                    <Input defaultValue={editingTemplate?.overall_stop_loss_bps ?? ""} inputMode="numeric" name="overallStopLossBps" />
                  </Field>
                  <Field label="Post-trigger action">
                    <Select defaultValue={editingTemplate?.post_trigger_action ?? "Stop"} name="postTriggerAction">
                      <option value="Stop">Stop</option>
                      <option value="Rebuild">Rebuild</option>
                    </Select>
                  </Field>
                  <Button type="submit">{editingTemplate ? "Save template changes" : "Create template"}</Button>
                </FormStack>
              ) : (
                <p>Template changes require super_admin.</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Template inventory</CardTitle>
          <CardDescription>Backend template records.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={canManageTemplates ? [
              { key: "name", label: "Name" },
              { key: "symbol", label: "Symbol" },
              { key: "market", label: "Market" },
              { key: "generation", label: "Generation" },
              { key: "levels", label: "Levels" },
              { key: "actions", label: "Actions" },
            ] : [
              { key: "name", label: "Name" },
              { key: "symbol", label: "Symbol" },
              { key: "market", label: "Market" },
              { key: "generation", label: "Generation" },
              { key: "levels", label: "Levels" },
            ]}
            rows={data.items.map((item) => ({
              actions: canManageTemplates ? (
                <form action="/admin/templates" method="get">
                  <input name="edit" type="hidden" value={item.id} />
                  <Button type="submit">Edit {item.name}</Button>
                </form>
              ) : null,
              id: item.id,
              generation: item.generation,
              levels: `${item.levels.length} levels`,
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
