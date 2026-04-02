import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminAddressPoolsData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ updated?: string }>;
};

export default async function AdminAddressPoolsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const updated = typeof params.updated === "string" ? params.updated : "";
  const data = await getAdminAddressPoolsData();

  return (
    <>
      {updated ? <StatusBanner description={"Updated address " + updated} title="Address pool updated" tone="success" /> : null}
      <AppShellSection
        description="Address inventory and enablement are read from backend billing address pools."
        eyebrow="Chain billing ops"
        title="Address Pool Inventory"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Add or enable address</CardTitle>
              <CardDescription>Submit address-level changes to the backend pool.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/admin/address-pools" method="post">
                <Field label="Chain">
                  <Select name="chain">
                    <option value="BSC">BSC</option>
                    <option value="ETH">ETH</option>
                    <option value="SOL">SOL</option>
                  </Select>
                </Field>
                <Field label="Address">
                  <Input name="address" placeholder="bsc-ops-1" />
                </Field>
                <input name="isEnabled" type="hidden" value="true" />
                <Button type="submit">Add or enable address</Button>
              </FormStack>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Address inventory</CardTitle>
          <CardDescription>Address-level backend pool records.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "chain", label: "Chain" },
              { key: "address", label: "Address" },
              { key: "enabled", label: "Enabled" },
              { key: "action", label: "Action" },
            ]}
            rows={data.addresses.map((item) => ({
              id: item.chain + ":" + item.address,
              action: (
                <FormStack action="/api/admin/address-pools" method="post">
                  <input name="chain" type="hidden" value={item.chain} />
                  <input name="address" type="hidden" value={item.address} />
                  <input name="isEnabled" type="hidden" value={item.is_enabled ? "false" : "true"} />
                  <Button type="submit">{(item.is_enabled ? "Disable " : "Enable ") + item.address}</Button>
                </FormStack>
              ),
              address: item.address,
              chain: item.chain,
              enabled: item.is_enabled ? "Yes" : "No",
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
