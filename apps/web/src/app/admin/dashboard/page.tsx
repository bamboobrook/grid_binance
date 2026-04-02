import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { Tabs } from "../../../components/ui/tabs";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

export default async function AdminDashboardPage() {
  const state = await getCurrentAdminProductState();
  const openMemberships = state.memberships.filter((item) => item.status === "Grace" || item.status === "Frozen").length;
  const openDeposits = state.deposits.filter((item) => item.state === "open").length;
  const runtimeIncidents = state.strategies.filter((item) => item.state === "error_paused").length;
  const activeTemplates = state.templates.filter((item) => item.status === "published").length;

  return (
    <>
      <StatusBanner
        description="Memberships, abnormal deposits, runtime incidents, and the latest audit trail stay visible in the operator overview."
        title="Admin queue summary"
        tone={openDeposits > 0 ? "danger" : "success"}
      />
      <AppShellSection
        actions={
          <Tabs
            activeHref="/admin/dashboard"
            items={[
              { href: "/admin/dashboard", label: "Overview" },
              { href: "/admin/deposits", label: "Deposits", badge: String(openDeposits) },
              { href: "/admin/audit", label: "Audit" },
            ]}
          />
        }
        description="An operations dashboard for membership risk, abnormal deposits, runtime incidents, and recent audit events."
        eyebrow="Admin overview"
        title="Admin Dashboard"
      >
        <div className="content-grid content-grid--metrics">
          <Card>
            <CardHeader>
              <CardTitle>Memberships needing action</CardTitle>
              <CardDescription>{openMemberships} operator-owned membership decisions</CardDescription>
            </CardHeader>
            <CardBody>
              <strong>{openMemberships} members in grace or frozen state</strong>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Deposit exception queue</CardTitle>
              <CardDescription>{openDeposits} deposit exceptions awaiting operator review</CardDescription>
            </CardHeader>
            <CardBody>
              <strong>{openDeposits} open abnormal cases</strong>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Runtime incidents</CardTitle>
              <CardDescription>{runtimeIncidents} strategies are not healthy</CardDescription>
            </CardHeader>
            <CardBody>
              <strong>{runtimeIncidents} error-paused strategies</strong>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Template catalog</CardTitle>
              <CardDescription>{activeTemplates} published templates in the shared catalog</CardDescription>
            </CardHeader>
            <CardBody>
              <strong>{activeTemplates} active templates</strong>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Operator attention board</CardTitle>
            <CardDescription>Direct links into the next operator action.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>
                <Link href="/admin/memberships">Membership review queue</Link>
              </li>
              <li>
                <Link href="/admin/deposits">Abnormal deposit handling</Link>
              </li>
              <li>
                <Link href="/admin/system">System confirmation thresholds</Link>
              </li>
              <li>Latest audit: {state.audit[0]?.action ?? "none"}</li>
            </ul>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Current health snapshot</CardTitle>
            <CardDescription>Numbers update as admin actions complete.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Open deposit cases currently: {openDeposits}</li>
              <li>Active template count: {activeTemplates}</li>
              <li>{state.addressPools.find((item) => item.chain === "bsc")?.total ?? 0} BSC pool addresses provisioned</li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Recent operator activity</CardTitle>
          <CardDescription>All critical admin actions remain audit-backed.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "timestamp", label: "Timestamp" },
              { key: "action", label: "Action" },
              { key: "target", label: "Target" },
              { key: "domain", label: "Domain", align: "right" },
            ]}
            rows={state.audit.slice(0, 6).map((entry) => ({
              id: entry.id,
              action: entry.action,
              domain: <Chip tone="info">{entry.domain}</Chip>,
              target: entry.target,
              timestamp: entry.timestamp,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
