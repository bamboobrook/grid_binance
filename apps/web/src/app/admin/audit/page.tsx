import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { getAdminAuditData } from "../../../lib/api/admin-product-state";

function payloadString(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null) {
    return "null";
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return "-";
}

function sessionSummary(payload: Record<string, unknown>) {
  const role = typeof payload.session_role === "string" && payload.session_role.length > 0 ? payload.session_role : "-";
  const sid = typeof payload.session_sid === "number" ? String(payload.session_sid) : "-";
  return `session role ${role} | session sid ${sid}`;
}

function beforeAfterSummary(payload: Record<string, unknown>) {
  const before = typeof payload.before_summary === "string" && payload.before_summary.length > 0 ? payload.before_summary : "-";
  const after = typeof payload.after_summary === "string" && payload.after_summary.length > 0 ? payload.after_summary : "-";
  return `before ${before} | after ${after}`;
}

function payloadSummary(payload: Record<string, unknown>) {
  const entries = Object.entries(payload);

  if (entries.length === 0) {
    return "-";
  }

  return entries.map(([key, value]) => `${key.replace(/_/g, " ")} ${payloadString(value)}`).join(" | ");
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
                  session: sessionSummary(item.payload),
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
                  payload: payloadSummary(item.payload),
                  summary: beforeAfterSummary(item.payload),
                }))}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
