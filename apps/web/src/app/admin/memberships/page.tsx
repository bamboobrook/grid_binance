import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentAdminProductState } from "../../../lib/api/admin-product-state";

type MembershipsPageProps = {
  searchParams?: Promise<{
    state?: string;
  }>;
};

export default async function AdminMembershipsPage({ searchParams }: MembershipsPageProps) {
  const params = (await searchParams) ?? {};
  const stateFilter = typeof params.state === "string" ? params.state : "all";
  const state = await getCurrentAdminProductState();
  const memberships = state.memberships.filter((item) => (stateFilter === "all" ? true : item.status.toLowerCase() === stateFilter));

  return (
    <>
      {state.flash.memberships ? (
        <StatusBanner
          description={state.flash.memberships.description}
          title={state.flash.memberships.title}
          tone={state.flash.memberships.tone}
        />
      ) : null}
      <AppShellSection
        description="Manual open, extend, freeze, unfreeze, and revoke decisions stay explicit, visible, and audit-backed."
        eyebrow="Membership operations"
        title="Membership Operations"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Filter membership queue</CardTitle>
              <CardDescription>Focus on grace, active, frozen, or revoked users.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/admin/memberships" method="get">
                <Field label="Membership state">
                  <Select defaultValue={stateFilter} name="state">
                    <option value="all">All states</option>
                    <option value="active">Active</option>
                    <option value="grace">Grace</option>
                    <option value="frozen">Frozen</option>
                    <option value="revoked">Revoked</option>
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
              <CardTitle>Operator rules</CardTitle>
              <CardDescription>Grace can continue running strategies, but starts stay blocked after the window ends.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Current filter: {stateFilter}</li>
                <li>Visible memberships: {memberships.length}</li>
                <li>Every action writes an audit entry with actor and target.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Membership queue</CardTitle>
          <CardDescription>Extend or review users without losing billing context.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "email", label: "User" },
              { key: "plan", label: "Plan" },
              { key: "status", label: "State" },
              { key: "expiresAt", label: "Expires" },
              { key: "note", label: "Note" },
              { key: "action", label: "Action", align: "right" },
            ]}
            rows={memberships.map((item) => ({
              id: item.id,
              action:
                item.email === "miles@example.com" ? (
                  <FormStack action="/api/admin/memberships" method="post">
                    <input name="membershipId" type="hidden" value={item.id} />
                    <Button type="submit">Extend miles@example.com by 30 days</Button>
                  </FormStack>
                ) : (
                  <Chip tone="info">Review only</Chip>
                ),
              email: item.email,
              expiresAt: item.expiresAt,
              note: item.note,
              plan: item.plan,
              status: <Chip tone={item.status === "Active" ? "success" : item.status === "Grace" ? "warning" : "danger"}>{item.status}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
