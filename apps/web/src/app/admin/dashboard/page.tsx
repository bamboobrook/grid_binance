import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import {
  getAdminAuditData,
  getAdminDepositsData,
  getAdminMembershipsData,
  getAdminStrategiesData,
  getCurrentAdminProfile,
  type AdminTemplateList,
  fetchAdminJson,
} from "../../../lib/api/admin-product-state";

export default async function AdminDashboardPage() {
  const profile = await getCurrentAdminProfile();
  const [memberships, deposits, strategies, audit, templates] = await Promise.all([
    getAdminMembershipsData(),
    getAdminDepositsData(),
    getAdminStrategiesData(),
    profile.admin_role === "super_admin" ? getAdminAuditData() : Promise.resolve({ items: [] }),
    profile.admin_permissions?.can_manage_templates ? fetchAdminJson<AdminTemplateList>("/admin/templates") : Promise.resolve({ items: [] }),
  ]);
  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipRisk = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const runtimeIncidents = strategies.items.filter((item) => item.status === "ErrorPaused").length;
  const role = profile.admin_role ?? "operator_admin";
  const restricted = role === "operator_admin";

  return (
    <>
      <StatusBanner
        description={restricted ? "Restricted permission boundary is active for this operator session." : "Super admin session is active for commercial control workflows."}
        title={profile.admin_access_granted ? "Admin access granted" : "Admin access missing"}
        tone={profile.admin_access_granted ? "success" : "warning"}
      />
      <AppShellSection
        description="Backend-backed operator summary for memberships, deposits, runtime state, and audits."
        eyebrow="Admin overview"
        title="Admin Dashboard"
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>Session role</CardTitle>
              <CardDescription>Current session privileges</CardDescription>
            </CardHeader>
            <CardBody>{restricted ? "Operator-only controls unlocked" : "Commercial control unlocked for treasury, pricing, and templates."}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Membership risk</CardTitle>
              <CardDescription>{membershipRisk} memberships need operator attention</CardDescription>
            </CardHeader>
            <CardBody>{membershipRisk}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Deposit exception queue</CardTitle>
              <CardDescription>{openDeposits} abnormal deposits are waiting for review</CardDescription>
            </CardHeader>
            <CardBody>{openDeposits}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Templates</CardTitle>
              <CardDescription>{templates.items.length} templates currently visible to this session</CardDescription>
            </CardHeader>
            <CardBody>{templates.items.length}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Runtime incidents</CardTitle>
              <CardDescription>{runtimeIncidents} strategies are error-paused</CardDescription>
            </CardHeader>
            <CardBody>{runtimeIncidents}</CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Recent operator activity</CardTitle>
          <CardDescription>Audit rows are read from backend storage.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "createdAt", label: "Timestamp" },
              { key: "action", label: "Action" },
              { key: "target", label: "Target" },
            ]}
            rows={audit.items.slice(0, 8).map((item, index) => ({
              id: `${item.action}-${index}`,
              action: item.action,
              createdAt: item.created_at.replace("T", " ").slice(0, 16),
              target: `${item.target_type}:${item.target_id}`,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
