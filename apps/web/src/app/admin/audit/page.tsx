import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Select } from "../../../components/ui/form";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

type AuditPageProps = {
  searchParams?: Promise<{
    domain?: string;
  }>;
};

export default async function AdminAuditPage({ searchParams }: AuditPageProps) {
  const params = (await searchParams) ?? {};
  const domainFilter = typeof params.domain === "string" ? params.domain : "all";
  const state = await getCurrentAdminProductState();
  const auditRows = state.audit.filter((item) => (domainFilter === "all" ? true : item.domain === domainFilter));

  return (
    <>
      <AppShellSection
        description="Review critical operator actions by domain, actor, target, and summary without leaving the admin console."
        eyebrow="Admin retention"
        title="Audit Log Review"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Filter audit records</CardTitle>
              <CardDescription>Narrow the log to the operator domain you are reviewing.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/audit" method="get">
                <Field label="Action domain">
                  <Select defaultValue={domainFilter} name="domain">
                    <option value="all">All domains</option>
                    <option value="membership">Membership</option>
                    <option value="deposit">Deposit</option>
                    <option value="pool">Pool</option>
                    <option value="template">Template</option>
                    <option value="system">System</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">Apply filters</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Required fields</CardTitle>
              <CardDescription>Every critical action keeps actor, timestamp, target, and summary context.</CardDescription>
            </CardHeader>
            <CardBody>
              Visible rows: {auditRows.length}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Recent log lines</CardTitle>
          <CardDescription>Filterable audit history for operator actions and system writes.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "timestamp", label: "Timestamp" },
              { key: "actor", label: "Actor" },
              { key: "action", label: "Action" },
              { key: "target", label: "Target" },
              { key: "summary", label: "Summary" },
              { key: "domain", label: "Domain", align: "right" },
            ]}
            rows={auditRows.map((item) => ({
              id: item.id,
              action: item.action,
              actor: item.actor,
              domain: <Chip tone="info">{item.domain}</Chip>,
              summary: item.summary,
              target: item.target,
              timestamp: item.timestamp,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
