import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { getAdminAddressPoolsData, getCurrentAdminProfile } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ updated?: string }>;
};

export default async function AdminAddressPoolsPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const updated = typeof query.updated === "string" ? query.updated : "";
  const [cookieStore, profile, data] = await Promise.all([cookies(), getCurrentAdminProfile(), getAdminAddressPoolsData()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const canManagePools = profile.admin_permissions?.can_manage_address_pools ?? false;
  const enabledCount = data.addresses.filter((item) => item.is_enabled).length;
  const pressureCount = data.addresses.length - enabledCount;

  return (
    <>
      {updated ? <StatusBanner description={pickText(lang, "已更新地址：" + updated, "Updated address: " + updated)} title={pickText(lang, "地址池已更新", "Address Pool Updated")} /> : null}
      <AppShellSection
        description={pickText(lang, "值班席位追踪地址池压力、链路分配和启停状态，避免充值入口在高峰期失去冗余。", "The desk tracks pool pressure, chain allocation, and enablement so billing routes do not lose redundancy under load.")}
        eyebrow={pickText(lang, "地址池治理", "Address Pool Governance")}
        title={pickText(lang, "地址池库存", "Address Pool Inventory")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "可用库存", "Enabled Inventory")}</CardTitle>
              <CardDescription>{pickText(lang, String(enabledCount) + " 个可用地址，" + String(pressureCount) + " 个停用或待补。", String(enabledCount) + " enabled addresses, " + String(pressureCount) + " disabled or pending replenishment.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {canManagePools ? (
                <FormStack action="/api/admin/address-pools" method="post">
                  <Field label={pickText(lang, "链路分配", "Chain Allocation")}>
                    <Select name="chain">
                      <option value="BSC">BSC</option>
                      <option value="ETH">ETH</option>
                      <option value="SOL">SOL</option>
                    </Select>
                  </Field>
                  <Field label={pickText(lang, "地址", "Address")}>
                    <Input name="address" placeholder="bsc-ops-1" />
                  </Field>
                  <input name="isEnabled" type="hidden" value="true" />
                  <Button type="submit">{pickText(lang, "新增或启用地址", "Add or Enable Address")}</Button>
                </FormStack>
              ) : (
                <p>{pickText(lang, "需要 super_admin 才能改地址池；当前席位只做压力观测。", "A super_admin session is required to change the address pool.")}</p>
              )}
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "值班说明", "Desk Notes")}</CardTitle>
              <CardDescription>{pickText(lang, "地址池压力和链路冗余必须显式暴露，不能等充值异常才回查。", "Pool pressure and route redundancy must stay explicit before deposit failures force a lookup.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>{pickText(lang, "停用地址会直接影响地址池压力指标。", "Disabled addresses immediately increase pool pressure.")}</li>
                <li>{pickText(lang, "链路分配应保持 ETH、BSC、SOL 都有冗余。", "Chain allocation should preserve redundancy on ETH, BSC, and SOL.")}</li>
                <li>{pickText(lang, "充值审核异常上升时，应优先检查对应链路是否只剩单地址。", "When deposit exceptions rise, first verify whether the affected chain is down to a single address.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "地址库存表", "Address Inventory")}</CardTitle>
          <CardDescription>{pickText(lang, "逐行展示链路、地址和启停动作。", "Shows chain, address, and enablement actions row by row.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "chain", label: pickText(lang, "链路", "Chain") },
              { key: "address", label: pickText(lang, "地址", "Address") },
              { key: "enabled", label: pickText(lang, "启用状态", "Enabled") },
              { key: "action", label: pickText(lang, "动作", "Action") },
            ]}
            rows={data.addresses.map((item) => ({
              id: item.chain + ":" + item.address,
              action: canManagePools ? (
                <FormStack action="/api/admin/address-pools" method="post">
                  <input name="chain" type="hidden" value={item.chain} />
                  <input name="address" type="hidden" value={item.address} />
                  <input name="isEnabled" type="hidden" value={item.is_enabled ? "false" : "true"} />
                  <Button type="submit">{item.is_enabled ? pickText(lang, "停用地址", "Disable Address") : pickText(lang, "启用地址", "Enable Address")}</Button>
                </FormStack>
              ) : pickText(lang, "受限", "Restricted"),
              address: item.address,
              chain: item.chain,
              enabled: item.is_enabled ? pickText(lang, "已启用", "Enabled") : pickText(lang, "已停用", "Disabled"),
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
