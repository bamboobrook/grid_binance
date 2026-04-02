import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminSweepsPage() {
  const state = await getCurrentAdminProductState();
  const queuedCount = state.sweeps.filter((item) => item.state === "Queued").length;

  return (
    <>
      <AppShellSection
        description="Treasury sweep visibility stays explicit so operators can confirm queue depth and completed collections without hidden automation."
        eyebrow="Treasury movement"
        title="Sweep Job Visibility"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Queued treasury jobs</CardTitle>
              <CardDescription>{queuedCount} jobs still waiting for execution.</CardDescription>
            </CardHeader>
            <CardBody>
              <strong>{queuedCount} queued sweep jobs</strong>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Audit requirement</CardTitle>
              <CardDescription>Every sweep action remains audit-backed.</CardDescription>
            </CardHeader>
            <CardBody>
              Latest sweep target: {state.sweeps[0]?.wallet ?? "n/a"}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Sweep queue</CardTitle>
          <CardDescription>Wallet, asset, amount, and execution state for treasury collection.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "wallet", label: "Wallet" },
              { key: "asset", label: "Asset" },
              { key: "amount", label: "Amount", align: "right" },
              { key: "requestedAt", label: "Requested at" },
              { key: "state", label: "State", align: "right" },
            ]}
            rows={state.sweeps.map((item) => ({
              id: item.id,
              amount: item.amount,
              asset: item.asset,
              requestedAt: item.requestedAt,
              state: <Chip tone={item.state === "Completed" ? "success" : "warning"}>{item.state}</Chip>,
              wallet: item.wallet,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
