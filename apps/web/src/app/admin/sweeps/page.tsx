import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminSweepsData } from "../../../lib/api/admin-product-state";

export default async function AdminSweepsPage() {
  const data = await getAdminSweepsData();

  return (
    <>
      <AppShellSection
        description="Treasury sweep jobs are read from backend billing sweep records."
        eyebrow="Treasury movement"
        title="Sweep Job Visibility"
      >
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
      </AppShellSection>
    </>
  );
}
