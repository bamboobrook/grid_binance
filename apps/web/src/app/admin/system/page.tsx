import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getAdminSystemData } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ bsc?: string; eth?: string; saved?: string; sol?: string }>;
};

export default async function AdminSystemPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const data = await getAdminSystemData();
  const hasSaved = params.saved === "1";
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
            <CardDescription>Edit all supported chain confirmation counts.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/admin/system" method="post">
              <Field label="ETH confirmations">
                <Input defaultValue={String(data.eth_confirmations)} inputMode="numeric" name="ethConfirmations" />
              </Field>
              <Field label="BSC confirmations">
                <Input defaultValue={String(data.bsc_confirmations)} inputMode="numeric" name="bscConfirmations" />
              </Field>
              <Field label="SOL confirmations">
                <Input defaultValue={String(data.sol_confirmations)} inputMode="numeric" name="solConfirmations" />
              </Field>
              <Button type="submit">Save confirmation policy</Button>
            </FormStack>
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
