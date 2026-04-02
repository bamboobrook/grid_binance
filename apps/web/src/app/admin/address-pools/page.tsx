import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminAddressPoolsPage() {
  const state = await getCurrentAdminProductState();

  return (
    <>
      {state.flash.addressPools ? (
        <StatusBanner
          description={state.flash.addressPools.description}
          title={state.flash.addressPools.title}
          tone={state.flash.addressPools.tone}
        />
      ) : null}
      <AppShellSection
        description="Expand address inventory before queues grow, while keeping one-hour lock visibility and chain-level utilization in view."
        eyebrow="Chain billing ops"
        title="Address Pool Expansion"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Expand pool capacity</CardTitle>
              <CardDescription>Each assigned address remains reserved for one hour before rotation can reuse it.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/address-pools" method="post">
                <Field label="Chain">
                  <Select defaultValue="bsc" name="chain">
                    <option value="ethereum">Ethereum</option>
                    <option value="bsc">BSC</option>
                    <option value="solana">Solana</option>
                  </Select>
                </Field>
                <Field label="Expand by">
                  <Input defaultValue="3" inputMode="numeric" name="expandBy" />
                </Field>
                <Button type="submit">Expand BSC pool</Button>
              </FormStack>
            </CardBody>
          </Card>
          <DialogFrame
            description="If the address pool is full, new payment orders must queue instead of reusing a locked address. Expansion is the operator-safe way to relieve pressure."
            title="Pool saturation rule"
            tone="warning"
          />
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Current pool pressure</CardTitle>
          <CardDescription>Visibility into total addresses, locks, and queued orders by chain.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "chain", label: "Chain" },
              { key: "total", label: "Total addresses", align: "right" },
              { key: "locked", label: "Locked", align: "right" },
              { key: "queue", label: "Queue", align: "right" },
            ]}
            rows={state.addressPools.map((item) => ({
              id: item.id,
              chain: item.chain === "bsc" ? "BSC" : item.chain === "solana" ? "Solana" : "Ethereum",
              locked: String(item.locked),
              queue: String(item.queue),
              total: String(item.total),
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
