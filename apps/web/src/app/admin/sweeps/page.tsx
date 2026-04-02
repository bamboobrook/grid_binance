import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminSweepsData, getCurrentAdminProfile } from "../../../lib/api/admin-product-state";

type PageProps = {
  searchParams?: Promise<{ submitted?: string; treasury?: string }>;
};

export default async function AdminSweepsPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const submitted = params.submitted === "1";
  const treasury = typeof params.treasury === "string" ? params.treasury : "";
  const [profile, data] = await Promise.all([getCurrentAdminProfile(), getAdminSweepsData()]);
  const canManageSweeps = profile.admin_permissions?.can_manage_sweeps ?? false;

  return (
    <>
      {submitted ? (
        <StatusBanner
          description={`Latest sweep destination recorded`}
          title="Sweep request submitted"
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Treasury sweep jobs are read from backend billing sweep records."
        eyebrow="Treasury movement"
        title="Sweep Operations"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Request treasury sweep</CardTitle>
              <CardDescription>Initiate admin-facing stablecoin sweep jobs.</CardDescription>
            </CardHeader>
            <CardBody>
              {canManageSweeps ? (
                <FormStack action="/api/admin/sweeps" method="post">
                  <input name="chain" type="hidden" value="BSC" />
                  <input name="asset" type="hidden" value="USDT" />
                  <Field label="Treasury address">
                    <Input name="treasuryAddress" placeholder="treasury-bsc-main" />
                  </Field>
                  <Field label="Source address">
                    <Input name="fromAddress" placeholder="bsc-addr-2" />
                  </Field>
                  <Field label="Sweep amount">
                    <Input name="amount" placeholder="18.50000000" />
                  </Field>
                  <Button type="submit">Request sweep</Button>
                </FormStack>
              ) : (
                <p>super_admin required for sweep operations.</p>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Queued treasury jobs</CardTitle>
          <CardDescription>Backend sweep jobs and treasury destinations.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "id", label: "Job" },
              { key: "chain", label: "Chain" },
              { key: "treasury", label: "Treasury" },
              { key: "status", label: "Status" },
            ]}
            rows={data.jobs.map((item) => ({
              id: String(item.sweep_job_id),
              chain: item.chain + " / " + item.asset,
              status: item.status,
              treasury: item.treasury_address,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
