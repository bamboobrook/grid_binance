import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getAdminSystemData, getCurrentAdminProfile } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ bsc?: string; eth?: string; saved?: string; sol?: string }>;
};

export default async function AdminSystemPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const [profile, data] = await Promise.all([getCurrentAdminProfile(), getAdminSystemData()]);
  const hasSaved = params.saved === "1";
  const canManageSystem = profile.admin_permissions?.can_manage_system ?? false;
  const eth = typeof params.eth === "string" ? params.eth : String(data.eth_confirmations);
  const bsc = typeof params.bsc === "string" ? params.bsc : String(data.bsc_confirmations);
  const sol = typeof params.sol === "string" ? params.sol : String(data.sol_confirmations);

  return (
    <>
      {hasSaved ? (
        <StatusBanner description={"ETH " + eth + " | BSC " + bsc + " | SOL " + sol} title="Confirmation policy saved" tone="success" />
      ) : null}
      <AppShellSection
        description="Per-chain confirmation policy is persisted in the backend system config store."
        eyebrow="System settings"
        title="System Configuration"
      >
        <Card>
          <CardHeader>
            <CardTitle>Confirmation policy</CardTitle>
            <CardDescription>
              {canManageSystem
                ? "Edit all supported chain confirmation counts."
                : "operator_admin sessions can review but cannot change confirmation counts."}
            </CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action={canManageSystem ? "/api/admin/system" : undefined} method="post">
              <Field label="ETH confirmations">
                <Input defaultValue={eth} disabled={!canManageSystem} inputMode="numeric" name="ethConfirmations" readOnly={!canManageSystem} />
              </Field>
              <Field label="BSC confirmations">
                <Input defaultValue={bsc} disabled={!canManageSystem} inputMode="numeric" name="bscConfirmations" readOnly={!canManageSystem} />
              </Field>
              <Field label="SOL confirmations">
                <Input defaultValue={sol} disabled={!canManageSystem} inputMode="numeric" name="solConfirmations" readOnly={!canManageSystem} />
              </Field>
              {canManageSystem ? <Button type="submit">Save confirmation policy</Button> : null}
              {!canManageSystem ? (
                <>
                  <p>Use a super_admin session to persist updated confirmation policy.</p>
                  <Button disabled type="button">
                    Save confirmation policy
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
