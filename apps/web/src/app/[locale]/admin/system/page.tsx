import { cookies } from "next/headers";

import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { getAdminSystemData, getCurrentAdminProfile } from "../../../../lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "../../../../lib/ui/preferences";

type PageProps = {
  searchParams?: Promise<{ bsc?: string; eth?: string; saved?: string; sol?: string }>;
};

export default async function AdminSystemPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const [cookieStore, profile, data] = await Promise.all([cookies(), getCurrentAdminProfile(), getAdminSystemData()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const hasSaved = params.saved === "1";
  const canManageSystem = profile.admin_permissions?.can_manage_system ?? false;
  const eth = typeof params.eth === "string" ? params.eth : String(data.eth_confirmations);
  const bsc = typeof params.bsc === "string" ? params.bsc : String(data.bsc_confirmations);
  const sol = typeof params.sol === "string" ? params.sol : String(data.sol_confirmations);

  return (
    <>
      {hasSaved ? (
        <StatusBanner description={pickText(lang, "ETH " + eth + "，BSC " + bsc + "，SOL " + sol, "ETH " + eth + ", BSC " + bsc + ", SOL " + sol)} title={pickText(lang, "确认数策略已保存", "Confirmation Policy Saved")} tone="success" />
      ) : null}
      <AppShellSection
        description={pickText(lang, "值班席位通过系统配置页维护各链确认数策略，并显式暴露变更影响和权限边界。", "The desk uses system configuration to maintain chain confirmation policy with explicit change impact and permission boundaries.")}
        eyebrow={pickText(lang, "系统配置", "System Configuration")}
        title={pickText(lang, "系统策略", "System Settings")}
      >
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "确认数策略", "Confirmation Policy")}</CardTitle>
            <CardDescription>
              {canManageSystem
                ? pickText(lang, "当前席位可直接修改 ETH、BSC、SOL 的确认数。", "This session can edit ETH, BSC, and SOL confirmation counts.")
                : pickText(lang, "当前席位只能查看确认数，不可保存变更。", "This session can review but cannot change confirmation counts.")}
            </CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action={canManageSystem ? "/api/admin/system" : undefined} method="post">
              <Field label={pickText(lang, "ETH 确认数", "ETH Confirmations")}>
                <Input defaultValue={eth} disabled={canManageSystem === false} inputMode="numeric" name="ethConfirmations" readOnly={canManageSystem === false} />
              </Field>
              <Field label={pickText(lang, "BSC 确认数", "BSC Confirmations")}>
                <Input defaultValue={bsc} disabled={canManageSystem === false} inputMode="numeric" name="bscConfirmations" readOnly={canManageSystem === false} />
              </Field>
              <Field label={pickText(lang, "SOL 确认数", "SOL Confirmations")}>
                <Input defaultValue={sol} disabled={canManageSystem === false} inputMode="numeric" name="solConfirmations" readOnly={canManageSystem === false} />
              </Field>
              {canManageSystem ? <Button type="submit">{pickText(lang, "保存确认数策略", "Save Confirmation Policy")}</Button> : null}
              {canManageSystem === false ? (
                <>
                  <p>{pickText(lang, "需要 super_admin 会话才能持久化系统配置变更。", "Use a super_admin session to persist updated confirmation policy.")}</p>
                  <Button disabled type="button">
                    {pickText(lang, "保存确认数策略", "Save Confirmation Policy")}
                  </Button>
                </>
              ) : null}
            </FormStack>
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
