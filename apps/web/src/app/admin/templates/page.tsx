import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminTemplatesData, getCurrentAdminProfile } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ created?: string }>;
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
  const profile = await getCurrentAdminProfile();
  const canManageTemplates = profile.admin_permissions?.can_manage_templates ?? false;
  const data = canManageTemplates ? await getAdminTemplatesData() : { items: [] };

  return (
    <>
      {created ? <StatusBanner description={`Created template name: ${created}`} title="Template created" tone="success" /> : null}
      <AppShellSection
        description="Template inventory is read from the backend strategy template catalog."
        eyebrow="Strategy templates"
        title="Template Management"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Create template</CardTitle>
              <CardDescription>Strategy template governance for the commercial catalog.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManageTemplates ? (
                <FormStack action="/api/admin/templates" method="post">
                  <Field label="Template name">
                    <Input name="name" placeholder="ADA Trend Rider" />
                  </Field>
                  <Field label="Symbol">
                    <Input defaultValue="ADAUSDT" name="symbol" />
                  </Field>
                  <Field label="Market">
                    <Select defaultValue="Spot" name="market">
                      <option value="Spot">Spot</option>
                      <option value="FuturesUsdM">FuturesUsdM</option>
                      <option value="FuturesCoinM">FuturesCoinM</option>
                    </Select>
                  </Field>
                  <Field label="Mode">
                    <Select defaultValue="SpotClassic" name="mode">
                      <option value="SpotClassic">SpotClassic</option>
                      <option value="SpotBuyOnly">SpotBuyOnly</option>
                      <option value="SpotSellOnly">SpotSellOnly</option>
                      <option value="FuturesLong">FuturesLong</option>
                      <option value="FuturesShort">FuturesShort</option>
                      <option value="FuturesNeutral">FuturesNeutral</option>
                    </Select>
                  </Field>
                  <Field label="Generation">
                    <Select defaultValue="Custom" name="generation">
                      <option value="Arithmetic">Arithmetic</option>
                      <option value="Geometric">Geometric</option>
                      <option value="Custom">Custom</option>
                    </Select>
                  </Field>
                  <Field label="Level 1 entry price">
                    <Input defaultValue="1.0000" name="level1EntryPrice" />
                  </Field>
                  <Field label="Level 1 quantity">
                    <Input defaultValue="10" name="level1Quantity" />
                  </Field>
                  <Field label="Level 1 take profit (bps)">
                    <Input defaultValue="150" inputMode="numeric" name="level1TakeProfitBps" />
                  </Field>
                  <Field label="Level 1 trailing (bps)">
                    <Input name="level1TrailingBps" />
                  </Field>
                  <Field label="Level 2 entry price">
                    <Input defaultValue="1.1000" name="level2EntryPrice" />
                  </Field>
                  <Field label="Level 2 quantity">
                    <Input defaultValue="10" name="level2Quantity" />
                  </Field>
                  <Field label="Level 2 take profit (bps)">
                    <Input defaultValue="180" inputMode="numeric" name="level2TakeProfitBps" />
                  </Field>
                  <Field label="Level 2 trailing (bps)">
                    <Input name="level2TrailingBps" />
                  </Field>
                  <Field label="Membership ready">
                    <Select defaultValue="true" name="membershipReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Exchange ready">
                    <Select defaultValue="true" name="exchangeReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Permissions ready">
                    <Select defaultValue="true" name="permissionsReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Withdrawals disabled">
                    <Select defaultValue="true" name="withdrawalsDisabled">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Hedge mode ready">
                    <Select defaultValue="true" name="hedgeModeReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Symbol ready">
                    <Select defaultValue="true" name="symbolReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Filters ready">
                    <Select defaultValue="true" name="filtersReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Margin ready">
                    <Select defaultValue="true" name="marginReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Conflict ready">
                    <Select defaultValue="true" name="conflictReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Balance ready">
                    <Select defaultValue="true" name="balanceReady">{readinessOptions()}</Select>
                  </Field>
                  <Field label="Overall take profit (bps)">
                    <Input inputMode="numeric" name="overallTakeProfitBps" />
                  </Field>
                  <Field label="Overall stop loss (bps)">
                    <Input inputMode="numeric" name="overallStopLossBps" />
                  </Field>
                  <Field label="Post-trigger action">
                    <Select defaultValue="Stop" name="postTriggerAction">
                      <option value="Stop">Stop</option>
                      <option value="Rebuild">Rebuild</option>
                    </Select>
                  </Field>
                  <Button type="submit">Create template</Button>
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
            columns={[
              { key: "name", label: "Name" },
              { key: "symbol", label: "Symbol" },
              { key: "market", label: "Market" },
              { key: "generation", label: "Generation" },
              { key: "levels", label: "Levels" },
            ]}
            rows={data.items.map((item) => ({
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
