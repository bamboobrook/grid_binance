import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminAddressPoolsData, getCurrentAdminProfile } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ updated?: string }>;
};

export default async function AdminAddressPoolsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const updated = typeof params.updated === "string" ? params.updated : "";
  const [profile, data] = await Promise.all([getCurrentAdminProfile(), getAdminAddressPoolsData()]);
  const canManagePools = profile.admin_permissions?.can_manage_address_pools ?? false;
  const enabledCount = data.addresses.filter((item) => item.is_enabled).length;

  return (
    <>
      {updated ? <StatusBanner description={`Updated address ${updated}`} title="Address pool updated" tone="success" /> : null}
      <AppShellSection
        description="Address inventory and enablement are read from backend billing address pools."
        eyebrow="Chain billing ops"
        title="Address Pool Inventory"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Enabled inventory</CardTitle>
              <CardDescription>{enabledCount} enabled addresses out of {data.addresses.length} total.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManagePools ? (
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
              ) : (
                <p>super_admin required for address inventory changes.</p>
              )}
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
              id: `${item.chain}:${item.address}`,
              action: canManagePools ? (
                <FormStack action="/api/admin/address-pools" method="post">
                  <input name="chain" type="hidden" value={item.chain} />
                  <input name="address" type="hidden" value={item.address} />
                  <input name="isEnabled" type="hidden" value={item.is_enabled ? "false" : "true"} />
                  <Button type="submit">{(item.is_enabled ? "Disable " : "Enable ") + item.address}</Button>
                </FormStack>
              ) : (
                "Restricted"
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
