import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminAuditData } from "../../../lib/api/admin-product-state";

export default async function AdminAuditPage() {
  const data = await getAdminAuditData();

  return (
    <>
      <AppShellSection
        description="Operator review is sourced from persisted backend audit rows."
        eyebrow="Admin retention"
        title="Audit Review"
      >
        <Card>
          <CardHeader>
            <CardTitle>Audit log</CardTitle>
            <CardDescription>Backend audit records are written server-side.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "time", label: "Time" },
                { key: "actor", label: "Actor" },
                { key: "action", label: "Action" },
                { key: "target", label: "Target" },
              ]}
              rows={data.items.map((item, index) => ({
                id: item.action + String(index),
                action: item.action,
                actor: item.actor_email,
                target: item.target_type + ":" + item.target_id,
                time: item.created_at.replace("T", " ").slice(0, 16),
              }))}
            />
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
