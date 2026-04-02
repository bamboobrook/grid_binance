import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getAdminUsersSnapshot } from "../../../lib/api/server";

export default async function AdminUsersPage() {
  const snapshot = await getAdminUsersSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="Filtering, overrides, and later audit-backed actions now share the same admin shell and form primitives."
        eyebrow="User operations"
        title="Member Control"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Filter members</CardTitle>
              <CardDescription>Reusable form primitives for future search and override actions.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="#" method="get">
                <Field label="Email search">
                  <Input name="email" placeholder="luna@example.com" />
                </Field>
                <Field label="Membership state">
                  <Select defaultValue="all" name="state">
                    <option value="all">All states</option>
                    <option value="active">Active</option>
                    <option value="grace">Grace</option>
                    <option value="frozen">Frozen</option>
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
              <CardTitle>Operator guidance</CardTitle>
              <CardDescription>Manual overrides must stay explicit and auditable.</CardDescription>
            </CardHeader>
            <CardBody>
              Freeze, unfreeze, extend, and revoke operations remain downstream work; this task establishes the shared shell and UI contract.
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>User state overview</CardTitle>
          <CardDescription>Preview table for membership and grace handling.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "email", label: "Email" },
              { key: "membership", label: "Membership" },
              { key: "grace", label: "Grace" },
              { key: "note", label: "Note" },
            ]}
            rows={snapshot.rows}
          />
        </CardBody>
      </Card>
    </>
  );
}
