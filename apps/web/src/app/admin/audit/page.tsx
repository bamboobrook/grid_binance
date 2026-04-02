import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminAuditData } from "../../../lib/api/admin-product-state";

function deriveSessionRole(actorEmail: string, payload: Record<string, unknown>) {
  if (typeof payload.session_role === "string" && payload.session_role.length > 0) {
    return payload.session_role;
  }
  if (actorEmail === "super-admin@example.com") {
    return "super_admin";
  }
  if (actorEmail === "admin@example.com") {
    return "operator_admin";
  }
  return "-";
}

function sessionSummary(actorEmail: string, payload: Record<string, unknown>) {
  const role = deriveSessionRole(actorEmail, payload);
  const sid = payload.session_sid;
  if (typeof sid === "number") {
    return `session role ${role} | session sid ${sid}`;
  }
  return `session role ${role}`;
}

function summarizePayload(actorEmail: string, payload: Record<string, unknown>) {
  const entries = Object.entries(payload);
  if (entries.length === 0) {
    return sessionSummary(actorEmail, payload);
  }

  const summaryEntries = [...entries.map(([key, value]) => `${key.replace(/_/g, " ")} ${String(value)}`)];
  if (!entries.some(([key]) => key === "session_role")) {
    summaryEntries.push(`session role ${deriveSessionRole(actorEmail, payload)}`);
  }
  return summaryEntries.join(" | ");
}

function afterSummary(action: string, payload: Record<string, unknown>) {
  if (action === "deposit.manual_credited") {
    return "decision credit_membership";
  }
  if (action === "deposit.manual_rejected") {
    return "decision reject";
  }
  const beforeSummary = payload.before_summary;
  const explicit = payload.after_summary;
  if (typeof beforeSummary === "string" && beforeSummary.length > 0 && typeof explicit === "string" && explicit.length > 0) {
    return `before ${beforeSummary} | after ${explicit}`;
  }
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
                  { key: "session", label: "Session" },
                  { key: "target", label: "Target" },
                ]}
                rows={data.items.map((item, index) => ({
                  id: item.action + String(index),
                  action: item.action,
                  actor: item.actor_email,
                  session: sessionSummary(item.actor_email, item.payload),
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
                  payload: summarizePayload(item.actor_email, item.payload),
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
