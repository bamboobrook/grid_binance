import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import {
  getAdminAuditData,
  getAdminDepositsData,
  getAdminMembershipsData,
  getAdminStrategiesData,
  getAdminTemplatesData,
  getCurrentAdminProfile,
} from "../../../lib/api/admin-product-state";

export default async function AdminDashboardPage() {
  const [profile, memberships, deposits, strategies, templates, audit] = await Promise.all([
    getCurrentAdminProfile(),
    getAdminMembershipsData(),
    getAdminDepositsData(),
    getAdminStrategiesData(),
    getAdminTemplatesData(),
    getAdminAuditData(),
  ]);
  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipRisk = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const runtimeIncidents = strategies.items.filter((item) => item.status === "ErrorPaused").length;

  return (
    <>
      <StatusBanner
        description={profile.admin_access_granted ? "Backend operator session is verified against the live admin API." : "Admin access is missing."}
        title={profile.admin_access_granted ? "Session verified" : "Admin access missing"}
        tone={profile.admin_access_granted ? "success" : "warning"}
      />
      <AppShellSection
        actions={<Tabs activeHref="/admin/dashboard" items={[{ href: "/admin/dashboard", label: "Overview" }, { href: "/admin/audit", label: "Audit" }]} />}
        description="Backend-backed operator summary for memberships, deposits, runtime state, and audits."
        eyebrow="Admin overview"
        title="Admin Dashboard"
      >
        <div className="content-grid content-grid--metrics">
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
              <CardTitle>Runtime incidents</CardTitle>
              <CardDescription>{runtimeIncidents} strategies are error-paused</CardDescription>
            </CardHeader>
            <CardBody>{runtimeIncidents}</CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Templates</CardTitle>
              <CardDescription>{templates.items.length} templates exist in the backend catalog</CardDescription>
            </CardHeader>
            <CardBody>{templates.items.length}</CardBody>
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
