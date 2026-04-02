import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminSystemPage() {
  const state = await getCurrentAdminProductState();

  return (
    <>
      {state.flash.system ? (
        <StatusBanner description={state.flash.system.description} title={state.flash.system.title} tone={state.flash.system.tone} />
      ) : null}
      <AppShellSection
        description="Manage billing confirmations and treasury-oriented settings with visible persistence and audit confirmation."
        eyebrow="System settings"
        title="System Configuration"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Billing configuration</CardTitle>
              <CardDescription>Confirmation thresholds and on-chain billing protections.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/system" method="post">
                <Field label="BSC confirmations">
                  <Input defaultValue={state.system.billing.bscConfirmations} inputMode="numeric" name="bscConfirmations" />
                </Field>
                <Button type="submit">Save billing configuration</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Treasury settings</CardTitle>
              <CardDescription>Read-only treasury references for this commercial recovery stage.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>BSC treasury: {state.system.treasury.bscWallet}</li>
                <li>Solana treasury: {state.system.treasury.solanaWallet}</li>
                <li>BSC USDT price: {state.system.treasury.usdtPriceBsc}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Current config snapshot</CardTitle>
          <CardDescription>Current effective values after the latest operator write.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "key", label: "Key" },
              { key: "value", label: "Value" },
              { key: "scope", label: "Scope", align: "right" },
            ]}
            rows={[
              { id: "cfg-bsc", key: "bsc_confirmations", scope: "Billing", value: state.system.billing.bscConfirmations },
              { id: "cfg-eth", key: "eth_confirmations", scope: "Billing", value: state.system.billing.ethConfirmations },
              { id: "cfg-sol", key: "solana_confirmations", scope: "Billing", value: state.system.billing.solanaConfirmations },
              { id: "cfg-wallet", key: "bsc_treasury_wallet", scope: "Treasury", value: state.system.treasury.bscWallet },
            ]}
          />
        </CardBody>
      </Card>
    </>
  );
}
