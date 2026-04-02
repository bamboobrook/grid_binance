import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminAuditData } from "../../../lib/api/admin-product-state";

function summarizePayload(payload: Record<string, unknown>) {
  const entries = Object.entries(payload);
  if (entries.length === 0) {
    return "-";
  }

  return entries
    .map(([key, value]) => `${key.replace(/_/g, " ")} ${String(value)}`)
    .join(" | ");
}

function afterSummary(action: string, payload: Record<string, unknown>) {
  if (action === "deposit.manual_credited") {
    return "decision credit_membership";
  }
  if (action === "deposit.manual_rejected") {
    return "decision reject";
  }
  const explicit = payload.after_summary;
  if (typeof explicit === "string" && explicit.length > 0) {
    return explicit;
  }
  const decision = payload.decision;
  if (typeof decision === "string") {
    return `decision ${decision}`;
  }
  const orderId = payload.order_id;
  if (typeof orderId === "number") {
    return `order ${orderId}`;
  }
  return "No before/after summary available.";
}

export default async function AdminAuditPage() {
  const data = await getAdminAuditData();

  return (
    <>
      <AppShellSection
        description="Operator review is sourced from persisted backend audit rows."
        eyebrow="Admin retention"
        title="Audit Review"
      >
        <div className="content-grid content-grid--split">
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
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Before / after summary</CardTitle>
              <CardDescription>Richer audit payload details surfaced for operator review.</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "action", label: "Action" },
                  { key: "summary", label: "Summary" },
                  { key: "payload", label: "Payload" },
                ]}
                rows={data.items.slice(0, 12).map((item, index) => ({
                  id: `summary-${index}`,
                  action: item.action,
                  payload: summarizePayload(item.payload),
                  summary: afterSummary(item.action, item.payload),
                }))}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
